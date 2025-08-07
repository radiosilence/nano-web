use anyhow::Result;
use handlebars::Handlebars;
use serde_json::json;
use std::collections::HashMap;
use std::env;

pub fn render_template(content: &str, config_prefix: &str) -> Result<String> {
    let handlebars = Handlebars::new();
    
    // Collect environment variables with the specified prefix
    let mut env_vars = HashMap::new();
    for (key, value) in env::vars() {
        if key.starts_with(config_prefix) {
            let trimmed_key = key.strip_prefix(config_prefix).unwrap();
            env_vars.insert(trimmed_key.to_string(), value);
        }
    }
    
    let json_string = serde_json::to_string(&env_vars)?;
    let escaped_json = json_string.replace('"', "\\\"");
    
    let data = json!({
        "env": env_vars,
        "json": json_string,
        "escapedJson": escaped_json
    });
    
    handlebars.render_template(content, &data)
        .map_err(|e| anyhow::anyhow!("Template rendering error: {}", e))
}