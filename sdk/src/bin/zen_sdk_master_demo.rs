use zen_engine::dataset::VirtualDataset;
use zen_engine::types::{AnalysisOptions, AnalysisLevel};
use zen_engine::analytics::join::JoinType;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let dir_path = PathBuf::from("/home/ejgi/Descargas/archivos/prueba/");
    
    println!("🚀 Zen Engine SDK: Master Demo");
    println!("-----------------------------");

    // 1. Carga de Dataset (Folder Mode)
    let mut options = AnalysisOptions::default();
    options.level = AnalysisLevel::Basic;
    options.no_limit = true; // Nitro Mode
    
    let mut dataset = VirtualDataset::new(&dir_path, &options)?;
    println!("✅ Dataset cargado: {} archivos, {} registros totales.", 
        dataset.engines.len(), dataset.get_total_rows());

    // 2. Mutabilidad Instantánea (Edición SDK)
    println!("\n[CRUD] Aplicando cambios en archivos masivos...");
    let target_file = "big_data_2gb.csv";
    dataset.update_row(target_file, 50, "9999,ZEN_SDK_DEMO,ACTIVE,MasterBranch".to_string())?;
    dataset.delete_row(target_file, 51)?;
    println!("✅ Registro 50 actualizado y 51 eliminado en {} (sin reescritura total).", target_file);

    // 3. Análisis con SIMD (Respetando los cambios anteriores)
    println!("\n[Analytics] Ejecutando agregación SIMD...");
    let stats = dataset.analyze(&options, None, None, None, None, None, None, None, None, None, None, None, 1)?;
    println!("✅ Agregación completada en registros reales.");
    println!("   - Total procesado: {} filas", stats.row_count);

    // 4. Join Relacional (SDK API)
    println!("\n[Join] Cruzando datos de Indicadores con Metadatos...");
    // Creamos un segundo dataset virtual para los metadatos si fuera necesario, 
    // pero aquí usaremos el mismo para simplificar (autounión o unión entre archivos del mismo repo)
    let join_results = dataset.join(&dataset, "Country Code", "Country Code", JoinType::Inner)?;
    println!("✅ Join completado: {} matches encontrados.", join_results.len());
    
    if let Some(first) = join_results.get(0) {
        println!("   Muestra: {} | {} ", first[0], first[1]);
    }

    println!("\n-----------------------------");
    println!("✨ Demo de SDK finalizada con éxito.");
    Ok(())
}
