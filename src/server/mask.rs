use crate::server::Server;

impl Server {
    pub fn mask_match(&self, host: &str, mask: &str) -> bool {
        // Implement IRC-style mask matching
        // Convert mask to regex pattern and match
        let pattern = mask.replace("*", ".*")
            .replace("?", ".")
            .replace("[", "\\[")
            .replace("]", "\\]");
        regex::Regex::new(&format!("^{}$", pattern))
            .map(|re| re.is_match(host))
            .unwrap_or(false)
    }
}