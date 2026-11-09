use url::Url;

/// An address on the web.
///
/// Guaranteed to be a valid URL with the `http` or `https` scheme whose
/// serialized form is at most 2048 characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct WebUrl(Url);

impl WebUrl {
    pub(crate) const MAX_LENGTH: usize = 2048;

    /// Parses text into a web URL.
    pub(crate) fn parse(text: &str) -> Result<Self, WebUrlParseError> {
        let url = match Url::parse(text) {
            Ok(url) => url,
            Err(error) => return Err(WebUrlParseError::Invalid(error)),
        };
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(WebUrlParseError::UnsupportedScheme {
                scheme: url.scheme().to_owned(),
            });
        }
        let length = url.as_str().len();
        if length > Self::MAX_LENGTH {
            return Err(WebUrlParseError::TooLong { length });
        }
        Ok(Self(url))
    }

    pub(crate) fn as_url(&self) -> &Url {
        &self.0
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum WebUrlParseError {
    #[error("web URL is not a valid URL")]
    Invalid(url::ParseError),
    #[error("web URL must use the http or https scheme, got {scheme}")]
    UnsupportedScheme { scheme: String },
    #[error(
        "web URL must be at most {} characters, got {length}",
        WebUrl::MAX_LENGTH
    )]
    TooLong { length: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_urls() {
        for text in ["http://example.com/", "https://quiz-works.com/about"] {
            assert_eq!(WebUrl::parse(text).unwrap().as_str(), text, "{text:?}");
        }
    }

    #[test]
    fn rejects_other_schemes() {
        assert_eq!(
            WebUrl::parse("ftp://example.com/"),
            Err(WebUrlParseError::UnsupportedScheme {
                scheme: "ftp".to_owned()
            })
        );
        assert_eq!(
            WebUrl::parse("javascript:alert(1)"),
            Err(WebUrlParseError::UnsupportedScheme {
                scheme: "javascript".to_owned()
            })
        );
    }

    #[test]
    fn rejects_text_that_is_not_a_url() {
        assert_eq!(
            WebUrl::parse("example.com"),
            Err(WebUrlParseError::Invalid(
                url::ParseError::RelativeUrlWithoutBase
            ))
        );
    }

    #[test]
    fn rejects_urls_over_the_limit() {
        let text = format!("https://example.com/{}", "x".repeat(2048));
        assert_eq!(
            WebUrl::parse(&text),
            Err(WebUrlParseError::TooLong { length: 2068 })
        );
    }
}
