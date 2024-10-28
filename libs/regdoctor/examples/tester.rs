use regdoctor::Database;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error")).init();

    let mimxrt1189_svd = std::fs::read_to_string("../../svds/MIMXRT1189_cm33-SecureExt.svd").unwrap();
    let cortex_m_svd = std::fs::read_to_string("../../svds/CortexM33.svd").unwrap();
    let mimxrt1189_svd = svd_parser::parse(&mimxrt1189_svd).unwrap();
    let cortex_m_svd = svd_parser::parse(&cortex_m_svd).unwrap();
    let mut db = Database::new();
    db.extend_with_svd(mimxrt1189_svd);
    db.extend_with_svd(cortex_m_svd);
    println!("DB len: {}", db.regs.len());
    let register_info = db.get_register(0x524C0000).unwrap();
    let register = register_info.decode_value(0x2);
    println!("{register:0X?}");
    let register_info = db.get_register(0x524C0020).unwrap();
    let register = register_info.decode_value(0x2);
    println!("{register:0X?}");
}
