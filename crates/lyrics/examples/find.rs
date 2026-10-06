//! `cargo run -p lyrics --example find -- "<title>" "<artist>" <seconds>`: asks every provider
//! and prints what each answered, which sheet wins and its translation.

use std::time::Duration;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let query = lyrics::LyricsQuery {
        title: args.next().unwrap_or_default(),
        artist: args.next().unwrap_or_default(),
        album: None,
        duration: Duration::from_secs(args.next().and_then(|s| s.parse().ok()).unwrap_or(0)),
        track: None,
    };
    let (sender, mut found) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(lyrics::gather(lyrics::providers(), query.clone(), sender));
    let mut hits = Vec::new();
    while let Some((_, batch)) = found.recv().await {
        for hit in &batch {
            println!(
                "{:14} synced={} worded={} {} - {}",
                hit.source,
                hit.lyrics.synced(),
                hit.lyrics.worded(),
                hit.title,
                hit.artist
            );
        }
        hits.extend(batch);
    }
    let ranked = lyrics::ordered(&query, hits);
    match ranked.first() {
        Some(best) => {
            println!("\nbest: {} (worded={})", best.source, best.lyrics.worded());
            if let lyrics::Lyrics::Synced { lines } = &best.lyrics {
                for line in lines.iter().take(6) {
                    println!("  [{:>6.2}] {}", line.start.as_secs_f64(), line.text);
                }
                match lyrics::translate(&query, lines).await {
                    Some(found) => {
                        println!(
                            "\ntranslation: {} (machine={})",
                            found.source, found.machine
                        );
                        for (line, translated) in lines.iter().zip(&found.lines).take(8) {
                            println!(
                                "  {}\n    → {}",
                                line.text,
                                translated.as_deref().unwrap_or("-")
                            );
                        }
                    }
                    None => println!("\nno translation"),
                }
            }
        }
        None => println!("no lyrics"),
    }
}
