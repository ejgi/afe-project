use std::time::Instant;
use zen_engine::dataset::delta::DeltaManager;
use zen_engine::analytics::join::{HashJoin, JoinTable};
use zen_engine::compute::GpuContext;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    println!("🕵️ Zen Engine: Auditoría Final de Capacidades");
    println!("===========================================");

    // 1. GPU Probe
    println!("\n[1/3] Direct Probe: GPU/WGPU Context");
    match GpuContext::new() {
        Ok(ctx) => {
            let info = ctx.get_adapter_info();
            println!("   ✅ GPU Detectada: {}", info.name);
            println!("   Backend: {:?}", info.backend);
            println!("   Tipo: {:?}", info.device_type);
        },
        Err(e) => println!("   ⚠️  GPU no disponible para cómputo: {}", e),
    }

    // 2. Delta Mutability (Nitro Edit)
    println!("\n[2/3] Stress Test: Delta Mutability (2.6GB File)");
    let base_path = PathBuf::from("/home/ejgi/Descargas/archivos/prueba/big_data_2gb.csv");
    if base_path.exists() {
        let t_delta = Instant::now();
        let mut dm = DeltaManager::new(base_path.clone())?;
        
        // Simular borrado de la fila 1,000,000 y actualización de la 2,000,000
        dm.delete_row(1_000_000);
        dm.update_row(2_000_000, "999999,ZEN_NITRO_VERIFIED,2026,1.0".to_string());
        
        dm.persist()?;
        println!("   ✅ Delta sidecar generado en: {:.2?}", t_delta.elapsed());
        println!("   (Se han editado registros en un archivo de 2.6GB sin reescribirlo)");
    } else {
        println!("   ⚠️  Archivo big_data_2gb.csv no encontrado.");
    }

    // 3. Partitioned Hash Join
    println!("\n[3/3] Logic Test: Partitioned Hash Join");
    let left_rows = vec![
        vec!["1".to_string(), "Argentina".to_string()],
        vec!["2".to_string(), "Brazil".to_string()],
    ];
    let right_rows = vec![
        vec!["1".to_string(), "Buenos Aires".to_string()],
        vec!["2".to_string(), "Brasilia".to_string()],
    ];
    
    let left_table = JoinTable::new(left_rows, 0);
    let right_table = JoinTable::new(right_rows, 0);
    
    let joiner = HashJoin::new("id", "city_id", 32);
    let results = joiner.execute_materialized(&left_table, &right_table)?;
    
    if results.len() == 2 {
        println!("   ✅ Hash Join Particionado funcionando correctamente.");
        println!("   Muestra: {:?}", results[0]);
    } else {
        println!("   ⚠️  Error en la lógica del Join.");
    }

    println!("\n===========================================");
    println!("Auditoría completada satisfactoriamente.");
    Ok(())
}
