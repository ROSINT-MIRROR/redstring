use crate::error::Error;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueFormat {
    Free,
    Ip,
    Hostname,
    Email,
    Url,
}

pub fn format_hint(format: ValueFormat) -> Result<&'static str, Error> {
    Ok(match format {
        ValueFormat::Free => "free text",
        ValueFormat::Ip => "IP address",
        ValueFormat::Hostname => "hostname",
        ValueFormat::Email => "email address",
        ValueFormat::Url => "http(s) URL",
    })
}

pub fn validate_value(format: ValueFormat, s: &str) -> Result<bool, Error> {
    let s = s.trim();
    Ok(match format {
        ValueFormat::Free => true,
        ValueFormat::Ip => s.parse::<std::net::IpAddr>().is_ok(),
        ValueFormat::Hostname => is_hostname(s),
        ValueFormat::Email => is_email(s),
        ValueFormat::Url => is_url(s),
    })
}

fn is_hostname(s: &str) -> bool {
    let s = s.to_lowercase();
    if s.is_empty() || s.len() > 253 || s.contains(' ') {
        return false;
    }
    let labels: Vec<&str> = s.split('.').collect();
    let Some(last) = labels.last() else {
        return false;
    };
    if !last.chars().any(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    labels.iter().all(|l| {
        !l.is_empty()
            && l.len() <= 63
            && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !l.starts_with('-')
            && !l.ends_with('-')
    })
}

fn is_email(s: &str) -> bool {
    if s.contains(' ') {
        return false;
    }
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty() && !domain.contains('@') && is_hostname(domain)
}

fn strip_scheme(s: &str) -> Option<&str> {
    let Some(r) = s.strip_prefix("https://") else {
        return s.strip_prefix("http://");
    };
    Some(r)
}

fn is_url(s: &str) -> bool {
    let Some(rest) = strip_scheme(s) else {
        return false;
    };
    let Some(host) = rest.split(['/', '?', '#']).next() else {
        return false;
    };
    let Some(host) = host.split(':').next() else {
        return false;
    };
    !rest.contains(' ') && (is_hostname(host) || host.parse::<std::net::IpAddr>().is_ok())
}

pub fn url_host(s: &str) -> Result<Option<String>, Error> {
    let Some(rest) = strip_scheme(s.trim()) else {
        return Ok(None);
    };
    let Some(host) = rest.split(['/', '?', '#']).next() else {
        return Ok(None);
    };
    let Some(host) = host.split(':').next() else {
        return Ok(None);
    };
    let host = host.trim_start_matches("www.");
    if host.is_empty() {
        return Ok(None);
    }
    Ok(Some(host.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_validators_accept_and_reject() {
        assert!(validate_value(ValueFormat::Ip, "1.1.1.1").unwrap());
        assert!(validate_value(ValueFormat::Ip, "2606:4700::1111").unwrap());
        assert!(!validate_value(ValueFormat::Ip, "1.1.1.999").unwrap());
        assert!(!validate_value(ValueFormat::Ip, "evil.example").unwrap());

        assert!(validate_value(ValueFormat::Hostname, "evil.example").unwrap());
        assert!(validate_value(ValueFormat::Hostname, "NS1.GOOGLE.COM").unwrap());
        assert!(!validate_value(ValueFormat::Hostname, "1.1.1.1").unwrap());
        assert!(!validate_value(ValueFormat::Hostname, "-bad.example").unwrap());
        assert!(!validate_value(ValueFormat::Hostname, "two words.com").unwrap());
        assert!(!validate_value(ValueFormat::Hostname, "").unwrap());

        assert!(validate_value(ValueFormat::Email, "a@b.example").unwrap());
        assert!(!validate_value(ValueFormat::Email, "a@b@c.example").unwrap());
        assert!(!validate_value(ValueFormat::Email, "nope").unwrap());

        assert!(validate_value(ValueFormat::Url, "https://evil.example/x?y#z").unwrap());
        assert!(validate_value(ValueFormat::Url, "http://1.1.1.1:8080/p").unwrap());
        assert!(!validate_value(ValueFormat::Url, "ftp://evil.example").unwrap());
        assert!(!validate_value(ValueFormat::Url, "evil.example").unwrap());

        assert!(validate_value(ValueFormat::Free, "anything at all").unwrap());
    }

    #[test]
    fn url_host_strips_scheme_path_port_and_www() {
        assert_eq!(url_host("https://github.com/torvalds").unwrap(), Some("github.com".to_owned()));
        assert_eq!(url_host("https://www.Reddit.com/user/x").unwrap(), Some("reddit.com".to_owned()));
        assert_eq!(url_host("http://host.example:8080/p?q#f").unwrap(), Some("host.example".to_owned()));
        assert_eq!(url_host("not a url").unwrap(), None);
        assert_eq!(url_host("ftp://x.example").unwrap(), None);
    }
}
