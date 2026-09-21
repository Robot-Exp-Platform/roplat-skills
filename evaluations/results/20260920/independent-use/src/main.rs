mod task_a;
mod task_b;
mod task_c;
#[tokio::main(flavor = "current_thread")]
async fn main() -> roplat::RoplatResult<()> {
    task_a::verify().await?;
    task_b::verify().await?;
    task_c::verify().await?;
    println!("PASS: independent skill tasks A, B, C");
    Ok(())
}
