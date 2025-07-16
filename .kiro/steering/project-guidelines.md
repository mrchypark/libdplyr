---
inclusion: always
---

# libdplyr 프로젝트 개발 가이드라인

## 프로젝트 개요
- **목적**: R의 dplyr 문법을 SQL 쿼리로 변환하는 Rust 기반 트랜스파일러
- **주요 기능**: 렉싱(Lexing) → 파싱(Parsing) → SQL 생성(Generation)
- **지원 방언**: PostgreSQL, MySQL, SQLite, Duckdb
- **아키텍처**: 모듈형 설계 (lexer, parser, sql_generator, error, cli)

## 코드 스타일 및 규칙

### Rust 코딩 컨벤션
- 모든 공개 API에 문서 주석(`///`) 필수 작성
- 에러 타입은 `thiserror` 크레이트 사용하여 정의
- 결과 타입은 `Result<T, E>` 형태로 일관성 있게 사용
- 테스트 코드는 각 모듈 하단에 `#[cfg(test)]` 블록으로 작성

### 에러 처리 패턴
- 각 단계별 전용 에러 타입 정의: `LexError`, `ParseError`, `GenerationError`
- 통합 에러 타입 `TranspileError`로 상위 레벨에서 처리
- 에러 메시지는 영어로 작성하되, 위치 정보 포함
- CLI에서는 사용자 친화적인 한국어 에러 메시지 제공

### 모듈 구조 원칙
- `src/lexer.rs`: 토큰화 담당
- `src/parser.rs`: AST 생성 담당  
- `src/sql_generator.rs`: SQL 변환 담당
- `src/error.rs`: 모든 에러 타입 정의
- `src/cli.rs`: 명령줄 인터페이스
- `src/lib.rs`: 공개 API 정의

## 개발 워크플로우

### 새로운 기능 추가 시
1. 해당하는 토큰을 `lexer.rs`에 추가
2. AST 노드를 `parser.rs`에 정의
3. SQL 생성 로직을 `sql_generator.rs`에 구현
4. 각 단계별 테스트 코드 작성
5. 통합 테스트를 `tests/integration_tests.rs`에 추가

### 테스트 전략
- 단위 테스트: 각 모듈별 기본 기능 검증
- 통합 테스트: 전체 변환 파이프라인 검증
- 벤치마크: 성능 회귀 방지를 위한 측정
- 방언별 테스트: PostgreSQL, MySQL, SQLite 각각 검증

## SQL 방언 지원

### 방언별 특성 고려사항
- **PostgreSQL**: 큰따옴표 식별자, `||` 문자열 연결
- **MySQL**: 백틱 식별자, `CONCAT()` 함수
- **SQLite**: 큰따옴표 식별자, `||` 문자열 연결

### 새로운 방언 추가 시
1. `SqlDialect` 트레이트 구현
2. 방언별 테스트 케이스 추가
3. CLI에서 방언 선택 옵션 업데이트
4. 문서 업데이트

## 성능 고려사항
- AST 노드는 `Clone` 트레이트 구현으로 효율적 복사 지원
- 문자열 처리 시 불필요한 할당 최소화
- 벤치마크를 통한 성능 회귀 모니터링
- 큰 입력에 대한 메모리 사용량 최적화

## 문서화 원칙
- 모든 공개 함수와 구조체에 Rustdoc 주석 작성
- 사용 예시 코드 포함
- 에러 조건과 반환값 명시
- README.md에 사용법과 예제 포함