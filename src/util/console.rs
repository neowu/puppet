use tokio::io::stdout;
use tokio::io::AsyncWriteExt;

use super::exception::Exception;

pub async fn print(text: &str) -> Result<(), Exception> {
    let out = &mut stdout();
    out.write_all(text.as_bytes()).await?;
    out.flush().await?;
    Ok(())
}
