use handl_platform::DefaultHost;

use handl_capability_build::BuildPlugin;
use handl_capability_compile::CompilePlugin;
use handl_capability_eval::EvalPlugin;
use handl_capability_extension::ExtensionManagerPlugin;
use handl_capability_fmt::FmtPlugin;
use handl_capability_kit::KitManagerPlugin;
use handl_capability_lint::LintPlugin;
use handl_capability_plugin::PluginManagerPlugin;
use handl_capability_typecheck::TypecheckPlugin;

fn main() {
    let mut host = DefaultHost::new();
    host.register_plugin(FmtPlugin::new());
    host.register_plugin(LintPlugin::new());
    host.register_plugin(TypecheckPlugin::new());
    host.register_plugin(EvalPlugin::new());
    host.register_plugin(CompilePlugin::new());
    host.register_plugin(BuildPlugin::new());
    host.register_plugin(ExtensionManagerPlugin::new());
    host.register_plugin(KitManagerPlugin::new());
    host.register_plugin(PluginManagerPlugin::new());

    let (code, msg) = match handl_cli::run(std::env::args().skip(1), &mut host) {
        Ok((0, out)) => (0, out),
        Ok((c, out)) => (c, out),
        Err(e) => (1, e.to_string()),
    };

    if code == 0 {
        println!("{msg}");
    } else {
        eprintln!("{msg}");
    }

    std::process::exit(code);
}
