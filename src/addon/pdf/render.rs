//! Page rasterizing through the Windows built-in PDF renderer (`Windows.Data.Pdf`).
//!
//! Nothing is bundled: Windows ships the renderer. A page is rendered into an in-memory
//! stream, decoded and handed to gpui as a [`RenderImage`]. All of it runs on one worker
//! thread, never on the UI thread, and the document is opened once and reused for every
//! page. Dropping the returned request sender ends the thread.
use async_channel::{Receiver, Sender};
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use windows::core::{Error, Interface as _, HSTRING};
use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
use windows::Foundation::IAsyncOperation;
use windows::Storage::StorageFile;
use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

/// A page to rasterize: which one, and the raster's pixel width. The height follows the
/// page's own shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub page: usize,
    pub width: u32,
}

/// What the worker thread sends to the view.
pub enum Msg {
    /// The document opened, or why it did not.
    Loaded(Result<Document, String>),
    /// One rasterized page, or why that page could not be rendered.
    Page {
        page: usize,
        width: u32,
        image: Result<Arc<RenderImage>, String>,
    },
}

/// A PDF the Windows renderer has opened: how many pages, and how big each one is.
pub struct Document {
    count: usize,
    sizes: Vec<(f32, f32)>,
}

impl Document {
    pub fn count(&self) -> usize {
        self.count
    }

    /// The page's size in points, the size it draws at zoom 1.
    pub fn size(&self, page: usize) -> (f32, f32) {
        self.sizes[page]
    }

    /// The widest page's width in points, which is what fit width divides.
    pub fn widest(&self) -> f32 {
        self.sizes.iter().map(|s| s.0).fold(0.0, f32::max)
    }
}

/// Start the render thread for `path`: the messages it sends back, and the queue of pages
/// to rasterize. Dropping the sender ends the thread.
pub fn start(path: &Path) -> (Receiver<Msg>, Sender<Request>) {
    let (requests, queue) = async_channel::unbounded::<Request>();
    let (msgs, incoming) = async_channel::unbounded::<Msg>();
    let unrunnable = msgs.clone();
    let path = path.to_path_buf();
    let started = std::thread::Builder::new()
        .name("smithy-pdf".into())
        .spawn(move || run(path, &queue, &msgs));
    if started.is_err() {
        // The view would wait on a thread that never runs, so answer it here.
        let _ = unrunnable.send_blocking(Msg::Loaded(Err(
            "Could not start the PDF renderer.".to_string()
        )));
    }
    (incoming, requests)
}

fn run(path: PathBuf, queue: &Receiver<Request>, msgs: &Sender<Msg>) {
    // WinRT wants an apartment. The blocking `get()` below needs one that does not pump
    // messages, so MTA; a thread that is already set up returns a code we can ignore.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let document = match open(&path) {
        Ok(document) => document,
        Err(reason) => {
            let _ = msgs.send_blocking(Msg::Loaded(Err(reason)));
            return;
        }
    };
    if msgs.send_blocking(Msg::Loaded(Ok(document.1))).is_err() {
        return;
    }
    while let Ok(request) = queue.recv_blocking() {
        let image = rasterize(&document.0, request);
        let page = Msg::Page {
            page: request.page,
            width: request.width,
            image,
        };
        if msgs.send_blocking(page).is_err() {
            break;
        }
    }
}

fn open(path: &Path) -> Result<(PdfDocument, Document), String> {
    // WinRT wants an absolute path with backslashes. A forward slash or a relative path
    // fails in `GetFileFromPathAsync` with a misleading error.
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let wide: Vec<u16> = absolute
        .as_os_str()
        .encode_wide()
        .map(|unit| {
            if unit == u16::from(b'/') {
                u16::from(b'\\')
            } else {
                unit
            }
        })
        .collect();
    let path = HSTRING::from_wide(&wide).map_err(opening)?;
    let file = StorageFile::GetFileFromPathAsync(&path)
        .and_then(|operation| operation.get())
        .map_err(opening)?;
    let document = PdfDocument::LoadFromFileAsync(&file)
        .and_then(|operation| operation.get())
        .map_err(opening)?;
    if document.IsPasswordProtected().map_err(opening)? {
        return Err("This PDF is password-protected, so it is not shown.".to_string());
    }
    let count = document.PageCount().map_err(opening)? as usize;
    if count == 0 {
        return Err("This PDF has no pages.".to_string());
    }
    // ponytail: every page's size is read up front so fit width is exact from the first
    // frame; upgrade: read sizes lazily and fit to the widest page met so far.
    let mut sizes = Vec::with_capacity(count);
    for index in 0..count {
        let page = document.GetPage(index as u32).map_err(opening)?;
        let size = page.Size().map_err(opening)?;
        let _ = page.Close();
        sizes.push((size.Width, size.Height));
    }
    Ok((document, Document { count, sizes }))
}

fn rasterize(document: &PdfDocument, request: Request) -> Result<Arc<RenderImage>, String> {
    let page = document.GetPage(request.page as u32).map_err(rendering)?;
    let stream = InMemoryRandomAccessStream::new().map_err(rendering)?;
    let options = PdfPageRenderOptions::new().map_err(rendering)?;
    options
        .SetDestinationWidth(request.width)
        .map_err(rendering)?;
    // The page is paper, not chrome: never repaint it in the system's contrast colours.
    options.SetIsIgnoringHighContrast(true).map_err(rendering)?;
    page.RenderWithOptionsToStreamAsync(&stream, &options)
        .and_then(|action| action.get())
        .map_err(rendering)?;
    let _ = page.Close();
    decode(&read(&stream)?)
}

/// The bytes the renderer wrote to the stream.
fn read(stream: &InMemoryRandomAccessStream) -> Result<Vec<u8>, String> {
    let size = stream.Size().map_err(rendering)? as usize;
    stream.Seek(0).map_err(rendering)?;
    let reader = DataReader::CreateDataReader(stream).map_err(rendering)?;
    let loaded: IAsyncOperation<u32> = reader
        .LoadAsync(size as u32)
        .map_err(rendering)?
        .cast()
        .map_err(rendering)?;
    loaded.get().map_err(rendering)?;
    let mut bytes = vec![0u8; size];
    reader.ReadBytes(&mut bytes).map_err(rendering)?;
    let _ = reader.Close();
    Ok(bytes)
}

/// Decode a rendered page into the BGRA pixels gpui draws from.
fn decode(bytes: &[u8]) -> Result<Arc<RenderImage>, String> {
    let rgba = image::load_from_memory(bytes)
        .map_err(|e| format!("Could not read the rendered page: {e}"))?
        .into_rgba8();
    let (width, height) = rgba.dimensions();
    let mut pixels = rgba.into_raw();
    for [blue, _, red, _] in pixels.as_chunks_mut::<4>().0 {
        std::mem::swap(blue, red);
    }
    let buffer = RgbaImage::from_raw(width, height, pixels).expect("the pixels came from a buffer");
    Ok(Arc::new(RenderImage::new([Frame::new(buffer)])))
}

fn opening(error: Error) -> String {
    let message = error.message();
    if message.is_empty() {
        // WinRT has no text for some codes, and a trailing colon reads as a bug.
        "Could not open this PDF: it is not a PDF, or it is damaged.".to_string()
    } else {
        format!("Could not open this PDF: {message}")
    }
}

fn rendering(error: Error) -> String {
    let message = error.message();
    if message.is_empty() {
        "Could not render this page.".to_string()
    } else {
        format!("Could not render this page: {message}")
    }
}
