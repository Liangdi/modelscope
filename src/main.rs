use clap::Parser;
use modelscope::ModelScope;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: SubCommand,
}
#[derive(Debug, Clone, Parser)]
enum SubCommand {
    /// Download model
    Download {
        /// Model ID
        #[arg(short, long)]
        model_id: String,
        /// The path to save the model, will be created if not exists
        #[arg(short, long)]
        save_dir: PathBuf,
    },
    /// Login to modelscope use your token
    Login {
        /// modelscope token
        #[arg(short, long)]
        token: String,
    },
    /// Logout
    Logout,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        SubCommand::Download { model_id, save_dir } => {
            ModelScope::download(&model_id, &save_dir).await?;
        }
        SubCommand::Login { token } => {
            ModelScope::login(&token).await?;
        }
        SubCommand::Logout => {
            ModelScope::logout().await?;
        }
    };

    Ok(())
}
