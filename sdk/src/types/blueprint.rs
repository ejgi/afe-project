use serde::{Serialize, Deserialize};
use super::schema::ColumnSchema;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FormulaOperand {
    Column(usize),
    Constant(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FormulaOp { Add, Sub, Mul, Div }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Formula {
    pub target_name: String,
    pub expression: String, // e.g., "col1 - col2"
    #[serde(skip)]
    pub left: Option<FormulaOperand>,
    #[serde(skip)]
    pub right: Option<FormulaOperand>,
    #[serde(skip)]
    pub op_code: Option<FormulaOp>,
}

impl Formula {
    pub fn compile(&mut self, headers: &[String]) {
        let parts: Vec<&str> = self.expression.split_whitespace().collect();
        if parts.len() == 3 {
            self.left = headers.iter().position(|h| h == parts[0])
                .map(FormulaOperand::Column)
                .or_else(|| parts[0].parse::<f64>().ok().map(FormulaOperand::Constant));
            
            self.right = headers.iter().position(|h| h == parts[2])
                .map(FormulaOperand::Column)
                .or_else(|| parts[2].parse::<f64>().ok().map(FormulaOperand::Constant));
            
            self.op_code = match parts[1] {
                "+" => Some(FormulaOp::Add),
                "-" => Some(FormulaOp::Sub),
                "*" => Some(FormulaOp::Mul),
                "/" => Some(FormulaOp::Div),
                _ => None,
            };
        }
    }

    #[inline(always)]
    pub fn evaluate_fast(&self, values: &[f64]) -> Option<f64> {
        let left_val = match self.left {
            Some(FormulaOperand::Column(idx)) => *values.get(idx)?,
            Some(FormulaOperand::Constant(c)) => c,
            None => return None,
        };
        let right_val = match self.right {
            Some(FormulaOperand::Column(idx)) => *values.get(idx)?,
            Some(FormulaOperand::Constant(c)) => c,
            None => return None,
        };

        match self.op_code {
            Some(FormulaOp::Add) => Some(left_val + right_val),
            Some(FormulaOp::Sub) => Some(left_val - right_val),
            Some(FormulaOp::Mul) => Some(left_val * right_val),
            Some(FormulaOp::Div) => if right_val != 0.0 { Some(left_val / right_val) } else { Some(0.0) },
            None => None,
        }
    }

    pub fn evaluate(&self, _headers: &[String], values: &[f64]) -> Option<f64> {
        self.evaluate_fast(values)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    pub schemas: Vec<ColumnSchema>,
    #[serde(default)]
    pub formulas: Vec<Formula>,
    #[serde(default)]
    pub discriminator_col: Option<usize>,
    #[serde(default)]
    pub regex_pattern: Option<String>,
    #[serde(default)]
    pub delimiter: Option<String>,
    #[serde(default)]
    pub rfc_4180: bool,
    #[serde(default)]
    pub skip_rows: usize,
    #[serde(default)]
    pub strip_quotes: bool,
}

impl Blueprint {
    pub fn hr_preset() -> Self {
        Self {
            schemas: vec![
                ColumnSchema::new("employee_id", super::schema::DataType::ID),
                ColumnSchema::new("full_name", super::schema::DataType::Category),
                ColumnSchema::new("email", super::schema::DataType::Email),
                ColumnSchema::new("salary", super::schema::DataType::Currency),
                ColumnSchema::new("hire_date", super::schema::DataType::Date),
            ],
            ..Default::default()
        }
    }

    pub fn finance_preset() -> Self {
        Self {
            schemas: vec![
                ColumnSchema::new("transaction_id", super::schema::DataType::ID),
                ColumnSchema::new("amount", super::schema::DataType::Currency),
                ColumnSchema::new("currency", super::schema::DataType::Category),
                ColumnSchema::new("source_account", super::schema::DataType::ID),
                ColumnSchema::new("destination_account", super::schema::DataType::ID),
                ColumnSchema::new("timestamp", super::schema::DataType::Date),
            ],
            ..Default::default()
        }
    }

    pub fn it_forensic_preset() -> Self {
        Self {
            schemas: vec![
                ColumnSchema::new("timestamp", super::schema::DataType::Date),
                ColumnSchema::new("source_ip", super::schema::DataType::IP),
                ColumnSchema::new("destination_ip", super::schema::DataType::IP),
                ColumnSchema::new("source_port", super::schema::DataType::Integer),
                ColumnSchema::new("protocol", super::schema::DataType::Category),
                ColumnSchema::new("uuid", super::schema::DataType::UUID),
                ColumnSchema::new("payload_json", super::schema::DataType::JSON),
            ],
            ..Default::default()
        }
    }
}

impl Default for Blueprint {
    fn default() -> Self {
        Self {
            schemas: Vec::new(),
            formulas: Vec::new(),
            discriminator_col: None,
            regex_pattern: None,
            delimiter: None,
            rfc_4180: false,
            skip_rows: 0,
            strip_quotes: false,
        }
    }
}
