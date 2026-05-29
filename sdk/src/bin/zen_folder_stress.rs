use std::time::Instant;
use zen_engine::dataset::VirtualDataset;
use zen_engine::dataset::delta::DeltaManager;
use zen_engine::analytics::join::{HashJoin, JoinTable};
use zen_engine::types::{AnalysisOptions, AnalysisLevel};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let dir_path = PathBuf::from("/home/ejgi/Descargas/archivos/prueba/");
    println!("🔥 Zen Engine: Stress Test de Carpeta Completa");
    println!("===========================================");

    // 1. Análisis Agregado (Folder Analytics)
    println!("\n[1/3] Agregación de Carpeta (SIMD + Multi-archivo)");
    let mut options = AnalysisOptions::default();
    options.level = AnalysisLevel::Basic;
    options.no_limit = true;
    
    let t_agg = Instant::now();
    let dataset = VirtualDataset::new(&dir_path, &options)?;
    let result = dataset.analyze(&options, None, None, None, None, None, None, None, None, None, None, None, 1)?;
    println!("   ✅ {} archivos procesados.", dataset.engines.len());
    println!("   ✅ Registros totales: {}", result.row_count);
    println!("   Tiempo: {:.2?}", t_agg.elapsed());

    // 2. Real-World Join (Data ⋈ Metadata)
    println!("\n[2/3] Join Real: Esperanza de Vida ⋈ Metadatos Región");
    let data_file = dir_path.join("API_SP.DYN.LE00.FE.IN_DS2_es_csv_v2_47550.csv");
    let meta_file = dir_path.join("Metadata_Country_API_SP.DYN.LE00.FE.IN_DS2_es_csv_v2_47550.csv");

    // Simular lectura de filas (simplificado para el test)
    let left_rows: Vec<Vec<String>> = std::fs::read_to_string(&data_file)?
        .lines().skip(4) // Skip header info
        .map(|l| l.split(',').map(|s| s.replace('"', "").to_string()).collect())
        .collect();
        
    let right_rows: Vec<Vec<String>> = std::fs::read_to_string(&meta_file)?
        .lines()
        .map(|l| l.split(',').map(|s| s.replace('"', "").to_string()).collect())
        .collect();

    let left_table = JoinTable::new(left_rows, 1); // Key: Country Code
    let right_table = JoinTable::new(right_rows, 1); // Key: Country Code
    
    let joiner = HashJoin::new("Country Code", "Code", 16);
    let joined = joiner.execute_materialized(&left_table, &right_table)?;
    
    println!("   ✅ Join completado: {} filas resultantes.", joined.len());
    if let Some(first) = joined.get(5) {
        println!("   Muestra (País + Región): {} -> {}", first[0], first[first.len()-2]);
    }

    // 3. Multi-File Delta Management
    println!("\n[3/3] Mutabilidad Multi-archivo (Delta)");
    let files_to_edit = vec![
        "big_data_2gb.csv",
        "system_logs.csv",
        "API_SP.DYN.LE00.FE.IN_DS2_es_csv_v2_47550.csv"
    ];

    for fname in files_to_edit {
        let fpath = dir_path.join(fname);
        let mut dm = DeltaManager::new(fpath)?;
        dm.update_row(10, "ZEN_FOLDER_STRESS_ACTIVE,9999,V1".to_string());
        dm.persist()?;
        println!("   ✅ Delta persistido para: {}", fname);
    }

    println!("\n===========================================");
    println!("Stress Test de Carpeta ¡EXITOSO!");
    Ok(())
}
