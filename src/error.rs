use thiserror::Error;

#[derive(Error, Debug)]
pub enum SearchError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Failed to parse HTML: {0}")]
    HtmlParse(String),

    #[error("No results found for query: {query}")]
    NoResults { query: String },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Cache error: {0}")]
    Cache(String),
}

pub type Result<T> = std::result::Result<T, SearchError>;

impl SearchError {
    /// 是否为 HTTP 错误
    pub fn is_http(&self) -> bool {
        matches!(self, SearchError::Http(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_messages() {
        let err = SearchError::NoResults {
            query: "test".to_string(),
        };
        assert_eq!(format!("{}", err), "No results found for query: test");

        let err = SearchError::HtmlParse("parse error".to_string());
        assert_eq!(format!("{}", err), "Failed to parse HTML: parse error");

        let err = SearchError::Config("missing key".to_string());
        assert_eq!(format!("{}", err), "Configuration error: missing key");

        let err = SearchError::Cache("io error".to_string());
        assert_eq!(format!("{}", err), "Cache error: io error");
    }

    #[test]
    fn test_result_type() {
        fn ok_fn() -> Result<i32> {
            Ok(42)
        }
        fn err_fn() -> Result<i32> {
            Err(SearchError::Config("bad config".to_string()))
        }

        assert_eq!(ok_fn().unwrap(), 42);
        assert!(err_fn().is_err());
    }
}
