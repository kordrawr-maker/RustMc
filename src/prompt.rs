use anyhow::Result;
use std::io::Write;

const SHOWN: usize = 20;

fn read_line() -> Result<String> {
    let mut out = std::io::stdout();
    out.write_all(b"> ")?;
    out.flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

pub fn menu_index(title: &str, options: &[String]) -> Result<usize> {
    println!("\n{title}");
    for (i, o) in options.iter().enumerate() {
        println!("  {:>2}. {o}", i + 1);
    }
    loop {
        match read_line()?.parse::<usize>() {
            Ok(n) if (1..=options.len()).contains(&n) => return Ok(n - 1),
            _ => println!("  pick a number between 1 and {}", options.len()),
        }
    }
}

pub fn menu_or_manual(title: &str, options: &[String]) -> Result<Option<String>> {
    let shown = SHOWN.min(options.len());
    println!("\n{title}");
    for (i, o) in options.iter().take(shown).enumerate() {
        println!("  {:>2}. {o}", i + 1);
    }
    let manual = shown + 1;
    println!("  {:>2}. enter manually", manual);
    if options.len() > shown {
        println!("  ({} more available via manual entry)", options.len() - shown);
    }
    loop {
        let t = read_line()?;
        if t.is_empty() {
            return Ok(options.first().cloned());
        }
        match t.parse::<usize>() {
            Ok(n) if n == manual => return Ok(None),
            Ok(n) if (1..=shown).contains(&n) => {
                return Ok(Some(options[n - 1].clone()));
            }
            _ => println!("  invalid choice"),
        }
    }
}

pub fn ask(prompt: &str, default: &str) -> Result<String> {
    print!("{prompt} [{default}]: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let t = line.trim().to_string();
    Ok(if t.is_empty() { default.to_string() } else { t })
}

pub fn yes_no(prompt: &str, default_yes: bool) -> Result<bool> {
    let suffix = if default_yes { "[Y/n]" } else { "[y/N]" };
    print!("{prompt} {suffix}: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    match line.trim().to_lowercase().as_str() {
        "" => Ok(default_yes),
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        _ => {
            println!("  please answer yes or no");
            yes_no(prompt, default_yes)
        }
    }
}
