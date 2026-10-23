use std::sync::Arc;

use lectrice_domain::{AppError, AppResult, DocumentReader, DocumentSummary, ErrorName, ListOptions, Page};
use tracing::info;

use crate::{RequestContext, UseCase};

/// The library, most recently opened first. The repository picks the page defaults; a limit given must be
/// between 1 and `ListOptions::MAX_LIMIT`.
pub struct ListRecentDocuments {
    reader: Arc<dyn DocumentReader>,
}

impl ListRecentDocuments {
    pub fn new(reader: Arc<dyn DocumentReader>) -> Self {
        Self { reader }
    }
}

impl UseCase for ListRecentDocuments {
    const NAME: &'static str = "ListRecentDocuments";
    type Args = ListOptions;
    type Output = Page<DocumentSummary>;

    fn run(&self, ctx: &RequestContext, args: ListOptions) -> AppResult<Page<DocumentSummary>> {
        info!(service = Self::NAME, request_id = %ctx.request_id, limit = args.limit, offset = args.offset, "listing recent documents");
        if !args.has_valid_limit() {
            return Err(AppError::new(ErrorName::RequestInvalid).with_detail(format!("limit {:?} is out of range", args.limit)));
        }

        let page = self.reader.list_recent(args)?;
        info!(service = Self::NAME, items = page.items.len(), total = page.total, limit = page.limit, offset = page.offset, "recent documents listed");
        Ok(page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::documents::fakes::Library;

    #[test]
    fn passes_the_page_through() {
        let library = Library::new(1);
        let id = library.add_book();

        let page = ListRecentDocuments::new(library.reader()).run(&RequestContext::new("test"), ListOptions::default()).unwrap();

        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, id);
        assert_eq!(page.items[0].title, "Fake Book");
    }

    /// Review A6: the service checks the limit itself, not only the route.
    #[test]
    fn a_limit_out_of_range_is_rejected() {
        let library = Library::new(1);
        let service = ListRecentDocuments::new(library.reader());
        for limit in [0, ListOptions::MAX_LIMIT + 1] {
            let error = service.run(&RequestContext::new("test"), ListOptions { limit: Some(limit), offset: None }).unwrap_err();
            assert_eq!(error.primary(), ErrorName::RequestInvalid);
        }
    }
}
