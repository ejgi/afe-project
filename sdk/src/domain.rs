use std::collections::HashMap;
use crate::types::BusinessTemplate;

pub struct AliasManager;

impl AliasManager {
    pub fn get_aliases(template: BusinessTemplate) -> HashMap<String, String> {
        let mut m = HashMap::new();
        match template {
            BusinessTemplate::Finance => {
                m.insert("sum".to_string(), "Volumen Total".to_string());
                m.insert("Avg".to_string(), "Ticket Promedio".to_string());
                m.insert("count".to_string(), "N° Operaciones".to_string());
                m.insert("std_dev".to_string(), "Volatilidad/Riesgo".to_string());
                m.insert("Min".to_string(), "Mínimo (H/L)".to_string());
                m.insert("Max".to_string(), "Máximo (H/L)".to_string());
                m.insert("Unique".to_string(), "Entidades Únicas".to_string());
            }
            BusinessTemplate::Network => {
                m.insert("sum".to_string(), "Bytes Totales / Tráfico".to_string());
                m.insert("count".to_string(), "N° Paquetes / Eventos".to_string());
                m.insert("Avg".to_string(), "Tamaño Promedio".to_string());
                m.insert("Unique".to_string(), "Hosts Únicos".to_string());
            }
            BusinessTemplate::Cybersecurity => {
                m.insert("count".to_string(), "Alertas / Logs".to_string());
                m.insert("Unique".to_string(), "IPs Atacantes".to_string());
                m.insert("sum".to_string(), "Payload Total".to_string());
            }
            BusinessTemplate::Sales => {
                m.insert("sum".to_string(), "Ingresos Totales".to_string());
                m.insert("Avg".to_string(), "Venta Promedio".to_string());
                m.insert("count".to_string(), "N° Ventas".to_string());
            }
            BusinessTemplate::General => {
                // No overrides for general
            }
        }
        m
    }

    pub fn translate(label: &str, aliases: &HashMap<String, String>) -> String {
        aliases.get(label).cloned().unwrap_or_else(|| label.to_string())
    }
}
