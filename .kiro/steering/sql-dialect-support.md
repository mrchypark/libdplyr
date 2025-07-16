---
inclusion: always
---

# SQL 방언 지원 및 확장성 가이드라인

## 지원 방언 현황

### 현재 지원 방언
- **PostgreSQL**: 표준 SQL 준수, 큰따옴표 식별자, `||` 문자열 연결
- **MySQL**: 백틱 식별자, `CONCAT()` 함수, 특수 함수 지원
- **SQLite**: 경량 데이터베이스, 제한된 기능, 큰따옴표 식별자
- **DuckDB**: 분석용 데이터베이스, PostgreSQL 호환성

### 방언별 특성 매트릭스

| 기능 | PostgreSQL | MySQL | SQLite | DuckDB |
|------|------------|-------|--------|--------|
| 식별자 인용 | `"name"` | `` `name` `` | `"name"` | `"name"` |
| 문자열 연결 | `\|\|` | `CONCAT()` | `\|\|` | `\|\|` |
| LIMIT 구문 | `LIMIT n` | `LIMIT n` | `LIMIT n` | `LIMIT n` |
| 집계 함수 | 표준 + 확장 | 표준 + 확장 | 기본만 | 표준 + 분석 |

## SqlDialect 트레이트 구현

### 필수 메서드
```rust
pub trait SqlDialect {
    /// 식별자 인용 (테이블명, 컬럼명)
    fn quote_identifier(&self, name: &str) -> String;
    
    /// 문자열 리터럴 인용
    fn quote_string(&self, value: &str) -> String;
    
    /// LIMIT 절 생성
    fn limit_clause(&self, limit: usize) -> String;
    
    /// 문자열 연결 연산
    fn string_concat(&self, left: &str, right: &str) -> String;
    
    /// 집계 함수 매핑
    fn aggregate_function(&self, function: &str) -> String;
    
    /// 대소문자 구분 여부
    fn is_case_sensitive(&self) -> bool;
    
    /// 방언 이름 반환
    fn dialect_name(&self) -> &'static str;
}
```

### 선택적 메서드 (기본 구현 제공)
```rust
pub trait SqlDialect {
    // ... 필수 메서드들 ...
    
    /// 날짜/시간 함수 매핑
    fn datetime_function(&self, function: &str) -> Option<String> {
        None // 기본적으로 지원하지 않음
    }
    
    /// 윈도우 함수 지원 여부
    fn supports_window_functions(&self) -> bool {
        true // 대부분의 현대 DB는 지원
    }
    
    /// CTE(Common Table Expression) 지원 여부
    fn supports_cte(&self) -> bool {
        true
    }
    
    /// JSON 함수 지원 여부
    fn supports_json_functions(&self) -> bool {
        false // 방언별로 다름
    }
}
```

## 새로운 방언 추가 프로세스

### 1. 방언 구조체 정의
```rust
#[derive(Debug, Clone)]
pub struct NewDialect {
    // 방언별 설정 필드들
    pub version: String,
    pub features: DialectFeatures,
}

#[derive(Debug, Clone)]
pub struct DialectFeatures {
    pub supports_json: bool,
    pub supports_arrays: bool,
    pub max_identifier_length: usize,
}
```

### 2. SqlDialect 트레이트 구현
```rust
impl SqlDialect for NewDialect {
    fn quote_identifier(&self, name: &str) -> String {
        // 방언별 구현
    }
    
    fn dialect_name(&self) -> &'static str {
        "newdialect"
    }
    
    // 기타 필수 메서드들...
}
```

### 3. CLI 통합
```rust
// src/cli.rs에서 방언 추가
impl std::str::FromStr for SqlDialectType {
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "postgresql" | "postgres" | "pg" => Ok(SqlDialectType::PostgreSql),
            "mysql" => Ok(SqlDialectType::MySql),
            "sqlite" => Ok(SqlDialectType::Sqlite),
            "duckdb" => Ok(SqlDialectType::DuckDB),
            "newdialect" => Ok(SqlDialectType::NewDialect), // 새 방언 추가
            _ => Err(format!("지원되지 않는 SQL 방언: {}", s)),
        }
    }
}
```

### 4. 테스트 케이스 추가
```rust
#[test]
fn test_new_dialect_basic_operations() {
    let transpiler = Transpiler::new(Box::new(NewDialect::default()));
    let dplyr_code = "select(name, age) %>% filter(age > 18)";
    
    let result = transpiler.transpile(dplyr_code);
    assert!(result.is_ok());
    
    let sql = result.unwrap();
    // 방언별 특성 검증
    assert!(sql.contains("방언별_특수_구문"));
}
```

## 방언별 최적화 전략

### 기능 감지 및 폴백
```rust
impl SqlGenerator {
    fn generate_with_fallback(&self, operation: &DplyrOperation) -> GenerationResult<String> {
        // 1차: 방언별 최적화된 구현 시도
        if let Ok(optimized) = self.generate_optimized(operation) {
            return Ok(optimized);
        }
        
        // 2차: 표준 SQL 폴백
        self.generate_standard(operation)
    }
}
```

### 방언별 함수 매핑 테이블
```rust
lazy_static! {
    static ref FUNCTION_MAPPINGS: HashMap<&'static str, HashMap<&'static str, &'static str>> = {
        let mut mappings = HashMap::new();
        
        // PostgreSQL 매핑
        let mut pg_map = HashMap::new();
        pg_map.insert("mean", "AVG");
        pg_map.insert("n", "COUNT(*)");
        mappings.insert("postgresql", pg_map);
        
        // MySQL 매핑
        let mut mysql_map = HashMap::new();
        mysql_map.insert("mean", "AVG");
        mysql_map.insert("n", "COUNT(*)");
        mappings.insert("mysql", mysql_map);
        
        mappings
    };
}
```

## 호환성 매트릭스 관리

### 기능 지원 체크
```rust
pub struct CompatibilityChecker {
    dialect: String,
    supported_features: HashSet<String>,
}

impl CompatibilityChecker {
    pub fn check_operation(&self, operation: &DplyrOperation) -> Result<(), GenerationError> {
        match operation {
            DplyrOperation::Summarise { aggregations } => {
                for agg in aggregations {
                    if !self.supports_aggregate(&agg.function) {
                        return Err(GenerationError::UnsupportedAggregateFunction {
                            function: agg.function.clone(),
                            dialect: self.dialect.clone(),
                        });
                    }
                }
            }
            // 다른 연산들도 체크...
        }
        Ok(())
    }
}
```

### 방언별 제한사항 문서화
```rust
/// 방언별 제한사항과 특이사항을 문서화
pub mod dialect_limitations {
    /// SQLite는 RIGHT JOIN을 지원하지 않음
    pub const SQLITE_NO_RIGHT_JOIN: &str = "SQLite does not support RIGHT JOIN";
    
    /// MySQL 5.7 이하는 윈도우 함수 미지원
    pub const MYSQL_OLD_NO_WINDOW: &str = "MySQL versions below 8.0 do not support window functions";
}
```

## 확장성 고려사항

### 플러그인 아키텍처 준비
```rust
pub trait DialectPlugin {
    fn name(&self) -> &'static str;
    fn create_dialect(&self) -> Box<dyn SqlDialect>;
    fn supported_features(&self) -> Vec<String>;
}

pub struct DialectRegistry {
    plugins: HashMap<String, Box<dyn DialectPlugin>>,
}
```

### 설정 기반 방언 커스터마이징
```rust
#[derive(Deserialize)]
pub struct DialectConfig {
    pub name: String,
    pub identifier_quote: String,
    pub string_quote: String,
    pub function_mappings: HashMap<String, String>,
    pub features: DialectFeatures,
}
```