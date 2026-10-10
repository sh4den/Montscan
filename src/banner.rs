use crate::ingress::Ingress;
use colored::Colorize;

use crate::config::Config;

pub struct IngressEntry {
    pub icon: &'static str,
    pub enabled: bool,
    pub flag: &'static str,
    pub ingress: Box<dyn Ingress>,
}

fn section(icon: &str, title: &str, lines: &[(&str, String)]) {
    println!("{}", format!("{icon} {title}:").green());
    for (i, (key, value)) in lines.iter().enumerate() {
        let branch = if i + 1 == lines.len() {
            "└─"
        } else {
            "├─"
        };
        println!("   {branch} {key}: {}", value.cyan());
    }
    println!();
}

fn disabled(title: &str, reason: &str) {
    println!("{}", format!("⚠️  {title}:").yellow());
    println!("   └─ {}", reason.yellow());
    println!();
}

pub fn print(cfg: &Config, ingresses: &[IngressEntry]) {
    for entry in ingresses {
        let name = entry.ingress.name();
        if entry.enabled {
            section(entry.icon, name, &entry.ingress.summary());
        } else {
            disabled(name, &format!("Disabled ({}=false)", entry.flag));
        }
    }

    section(
        "🤖",
        "AI Processing (Ollama)",
        &[
            ("Host", cfg.ollama.host.clone()),
            ("Model", cfg.ollama.model.clone()),
            ("Timeout", format!("{}s", cfg.ollama.timeout.as_secs())),
        ],
    );

    section(
        "📄",
        "PDF Processing",
        &[("Renderer", "hayro (built-in)".to_string())],
    );

    let rule = "──────────────────────────────────────────────────────────────────────".cyan();
    println!("{rule}");
    println!(
        "{}",
        "✅ All systems initialized - Ready to process documents!".green()
    );
    println!("{rule}");
    println!();
}
