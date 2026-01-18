use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;

use juno_keys::{KeysError, Network};

const JSON_VERSION: &str = "v1";

#[derive(Parser)]
#[command(
    name = "juno-keys",
    about = "Seed + UFVK derivation for Juno Cash",
    version
)]
struct Cli {
    #[arg(long, help = "JSON output (stable)")]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Seed {
        #[command(subcommand)]
        command: SeedCmd,
    },
    #[command(name = "ufvk")]
    UFVK {
        #[command(subcommand)]
        command: UfvkCmd,
    },
}

#[derive(Subcommand)]
enum SeedCmd {
    #[command(name = "new")]
    New(SeedNewArgs),
}

#[derive(Args)]
struct SeedNewArgs {
    #[arg(
        long,
        default_value_t = 64,
        help = "Seed size in bytes (ZIP32 allows 32..252)"
    )]
    bytes: usize,

    #[arg(long, help = "Write seed (base64) to a file (mode 0600 on unix)")]
    out: Option<PathBuf>,

    #[arg(long, help = "Overwrite --out if it exists")]
    force: bool,

    #[arg(long, help = "Print seed to stdout (warning: avoid logs)")]
    print: bool,
}

#[derive(Subcommand)]
enum UfvkCmd {
    #[command(name = "from-seed")]
    FromSeed(UfvkFromSeedArgs),
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum NetworkArg {
    Mainnet,
    Testnet,
    Regtest,
}

impl From<NetworkArg> for Network {
    fn from(v: NetworkArg) -> Self {
        match v {
            NetworkArg::Mainnet => Network::Mainnet,
            NetworkArg::Testnet => Network::Testnet,
            NetworkArg::Regtest => Network::Regtest,
        }
    }
}

#[derive(Args)]
struct UfvkFromSeedArgs {
    #[arg(long, help = "Read seed base64 from a file")]
    seed_file: Option<PathBuf>,

    #[arg(long, help = "Seed as base64 (warning: avoid logs)")]
    seed_base64: Option<String>,

    #[arg(long, value_enum, help = "Network selection (sets ua_hrp + coin_type)")]
    network: NetworkArg,

    #[arg(long, default_value_t = 0, help = "Account (typically 0)")]
    account: u32,
}

#[derive(Debug)]
enum AppError {
    InvalidRequest(String),
    Io(String),
    Keys(KeysError),
}

impl AppError {
    fn code(&self) -> &'static str {
        match self {
            AppError::InvalidRequest(_) => "invalid_request",
            AppError::Io(_) => "io_error",
            AppError::Keys(e) => e.code(),
        }
    }

    fn message(&self) -> String {
        match self {
            AppError::InvalidRequest(s) => s.clone(),
            AppError::Io(s) => s.clone(),
            AppError::Keys(e) => e.to_string(),
        }
    }
}

#[derive(Serialize)]
struct OkEnvelope<T: Serialize> {
    version: &'static str,
    status: &'static str,
    data: T,
}

#[derive(Serialize)]
struct ErrEnvelope {
    version: &'static str,
    status: &'static str,
    error: ErrObj,
}

#[derive(Serialize)]
struct ErrObj {
    code: String,
    message: String,
}

fn main() {
    let cli = Cli::parse();
    let exit_code = match run(&cli) {
        Ok(()) => 0,
        Err(e) => {
            write_error(&cli, &e);
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(cli: &Cli) -> Result<(), AppError> {
    match &cli.command {
        Command::Seed {
            command: SeedCmd::New(args),
        } => cmd_seed_new(cli, args),
        Command::UFVK {
            command: UfvkCmd::FromSeed(args),
        } => cmd_ufvk_from_seed(cli, args),
    }
}

fn cmd_seed_new(cli: &Cli, args: &SeedNewArgs) -> Result<(), AppError> {
    let seed_b64 = juno_keys::generate_seed_base64(args.bytes).map_err(AppError::Keys)?;

    let out_path = if let Some(out) = &args.out {
        write_secret_file(out, &(seed_b64.as_str().to_string() + "\n"), args.force)?;
        Some(out.clone())
    } else {
        None
    };

    let should_print = args.print || out_path.is_none();

    if cli.json {
        #[derive(Serialize)]
        struct SeedOut {
            bytes: usize,
            #[serde(skip_serializing_if = "Option::is_none")]
            out_path: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            seed_base64: Option<String>,
        }
        let data = SeedOut {
            bytes: args.bytes,
            out_path: out_path.as_ref().map(|p| p.display().to_string()),
            seed_base64: if should_print {
                Some(seed_b64.as_str().to_string())
            } else {
                None
            },
        };
        write_json_ok(&data)?;
        return Ok(());
    }

    if should_print {
        println!("{}", seed_b64.as_str());
        return Ok(());
    }

    if let Some(p) = out_path {
        println!("{}", p.display());
    }
    Ok(())
}

fn cmd_ufvk_from_seed(cli: &Cli, args: &UfvkFromSeedArgs) -> Result<(), AppError> {
    let seed_b64 = match (&args.seed_file, &args.seed_base64) {
        (Some(_), Some(_)) => {
            return Err(AppError::InvalidRequest(
                "use either --seed-file or --seed-base64 (not both)".to_string(),
            ))
        }
        (None, None) => {
            return Err(AppError::InvalidRequest(
                "missing seed (set --seed-file or --seed-base64)".to_string(),
            ))
        }
        (Some(p), None) => read_seed_file(p)?,
        (None, Some(s)) => s.trim().to_string(),
    };

    let net: Network = args.network.into();
    let ua_hrp = net.ua_hrp();
    let coin_type = net.coin_type();
    let ufvk = juno_keys::ufvk_from_seed_base64(&seed_b64, ua_hrp, coin_type, args.account)
        .map_err(AppError::Keys)?;

    if cli.json {
        #[derive(Serialize)]
        struct UfvkOut {
            ufvk: String,
            ua_hrp: &'static str,
            coin_type: u32,
            account: u32,
        }
        let data = UfvkOut {
            ufvk,
            ua_hrp,
            coin_type,
            account: args.account,
        };
        write_json_ok(&data)?;
        return Ok(());
    }

    println!("{ufvk}");
    Ok(())
}

fn read_seed_file(path: &Path) -> Result<String, AppError> {
    let raw = fs::read_to_string(path).map_err(|e| AppError::Io(format!("read seed file: {e}")))?;
    let v = raw.trim().to_string();
    if v.is_empty() {
        return Err(AppError::Keys(KeysError::SeedInvalid));
    }
    Ok(v)
}

fn write_secret_file(path: &Path, contents: &str, force: bool) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| AppError::Io(format!("create dir: {e}")))?;
        }
    }

    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;

        let mut opts = OpenOptions::new();
        opts.write(true);
        if force {
            opts.create(true).truncate(true);
        } else {
            opts.create_new(true);
        }
        opts.mode(0o600);
        let mut f = opts
            .open(path)
            .map_err(|e| AppError::Io(format!("open file: {e}")))?;
        f.write_all(contents.as_bytes())
            .map_err(|e| AppError::Io(format!("write file: {e}")))?;
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        if !force && path.exists() {
            return Err(AppError::Io("file exists".to_string()));
        }
        fs::write(path, contents).map_err(|e| AppError::Io(format!("write file: {e}")))?;
        Ok(())
    }
}

fn write_json_ok<T: Serialize>(data: &T) -> Result<(), AppError> {
    let env = OkEnvelope {
        version: JSON_VERSION,
        status: "ok",
        data,
    };
    serde_json::to_writer(io::stdout(), &env)
        .map_err(|e| AppError::Io(format!("json encode: {e}")))?;
    println!();
    Ok(())
}

fn write_error(cli: &Cli, err: &AppError) {
    if cli.json {
        let env = ErrEnvelope {
            version: JSON_VERSION,
            status: "err",
            error: ErrObj {
                code: err.code().to_string(),
                message: err.message(),
            },
        };
        let _ = serde_json::to_writer(io::stdout(), &env);
        let _ = println!();
        return;
    }

    let _ = writeln!(io::stderr(), "{}", err.message());
}
