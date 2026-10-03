//! The page bitmaps held in memory.
//!
//! A page is rasterized at a pixel width, so a bitmap rendered for another width (after
//! a zoom or a resize) is not reused. Pages are kept least-recently-seen: the list asks
//! for the pages it can see on every frame, so what is dropped is what has scrolled away.
//! Without that, a long document would hold every page at a few megabytes each.
use gpui::RenderImage;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

pub struct PageCache {
    /// Page index -> the pixel width it was rendered at, and the bitmap.
    pages: HashMap<usize, (u32, Arc<RenderImage>)>,
    /// The pages that hold a bitmap, oldest request first. Keys match `pages` exactly.
    seen: VecDeque<usize>,
    limit: usize,
}

impl PageCache {
    pub fn new(limit: usize) -> Self {
        Self {
            pages: HashMap::new(),
            seen: VecDeque::new(),
            limit: limit.max(1),
        }
    }

    /// The bitmap for `page` if it was rendered at exactly `width`, marked as seen.
    pub fn get(&mut self, page: usize, width: u32) -> Option<Arc<RenderImage>> {
        match self.pages.get(&page) {
            Some((w, image)) if *w == width => {
                let image = Arc::clone(image);
                self.touch(page);
                Some(image)
            }
            Some(_) => {
                // Rendered at a width that no longer fits: drop it so it is asked for
                // again at the right size.
                self.forget(page);
                None
            }
            None => None,
        }
    }

    /// Keep `image` for `page`, dropping the least recently seen pages.
    pub fn insert(&mut self, page: usize, width: u32, image: Arc<RenderImage>) {
        self.touch(page);
        self.pages.insert(page, (width, image));
        while self.pages.len() > self.limit {
            if let Some(oldest) = self.seen.pop_front() {
                self.pages.remove(&oldest);
            }
        }
    }

    fn touch(&mut self, page: usize) {
        if let Some(at) = self.seen.iter().position(|p| *p == page) {
            self.seen.remove(at);
        }
        self.seen.push_back(page);
    }

    fn forget(&mut self, page: usize) {
        self.pages.remove(&page);
        if let Some(at) = self.seen.iter().position(|p| *p == page) {
            self.seen.remove(at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::RenderImage;
    use image::{Frame, RgbaImage};

    fn page() -> Arc<RenderImage> {
        let buffer = RgbaImage::new(1, 1);
        Arc::new(RenderImage::new([Frame::new(buffer)]))
    }

    #[test]
    fn drops_the_least_recently_seen_page() {
        let mut cache = PageCache::new(2);
        cache.insert(0, 10, page());
        cache.insert(1, 10, page());
        // Looking at page 0 again makes page 1 the oldest.
        assert!(cache.get(0, 10).is_some());
        cache.insert(2, 10, page());
        assert!(cache.get(1, 10).is_none());
        assert!(cache.get(0, 10).is_some());
        assert!(cache.get(2, 10).is_some());
    }

    #[test]
    fn a_bitmap_rendered_at_another_width_is_not_reused() {
        let mut cache = PageCache::new(4);
        cache.insert(0, 10, page());
        assert!(cache.get(0, 20).is_none());
        // The stale bitmap is gone rather than kept alongside the next one.
        assert!(cache.get(0, 10).is_none());
    }

    #[test]
    fn reinserting_a_page_does_not_grow_the_bookkeeping() {
        let mut cache = PageCache::new(2);
        for _ in 0..5 {
            cache.insert(0, 10, page());
        }
        assert_eq!(cache.pages.len(), 1);
        assert_eq!(cache.seen.len(), 1);
    }
}
