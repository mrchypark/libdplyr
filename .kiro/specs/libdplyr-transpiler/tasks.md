# Implementation Plan

- [x] 1. 프로젝트 구조 설정 및 기본 의존성 구성
  - Cargo.toml 파일 생성 및 필요한 의존성 추가 (clap, thiserror, criterion 등)
  - 기본 모듈 구조 생성 (lib.rs, lexer.rs, parser.rs, sql_generator.rs, cli.rs)
  - 프로젝트 디렉토리 구조 설정 (src/, tests/, benches/)
  - _Requirements: 4.1, 5.1_

- [x] 2. 기본 오류 타입 및 공통 데이터 구조 구현
  - LexError, ParseError, GenerationError, TranspileError 열거형 정의
  - thiserror 매크로를 사용한 오류 타입 구현
  - 기본 AST 노드 구조체들 정의 (DplyrNode, DplyrOperation, Expr 등)
  - _Requirements: 1.3, 2.3_

- [x] 3. Lexer 모듈 구현
  - Token 열거형 정의 (dplyr 함수, 연산자, 리터럴, 구조 토큰들)
  - Lexer 구조체 구현 및 토큰화 로직 작성
  - 문자열, 숫자, 식별자 파싱 기능 구현
  - 파이프 연산자 (%>%) 및 기타 dplyr 특화 토큰 처리
  - _Requirements: 2.1, 2.2_

- [x] 4. Lexer 단위 테스트 작성
  - 기본 토큰 파싱 테스트 (식별자, 문자열, 숫자)
  - dplyr 함수 토큰 인식 테스트 (select, filter, mutate 등)
  - 파이프 연산자 및 특수 문자 처리 테스트
  - 오류 케이스 테스트 (잘못된 문자, 미완성 문자열 등)
  - _Requirements: 6.1, 6.3_

- [x] 5. Parser 모듈 기본 구조 구현
  - Parser 구조체 및 기본 파싱 메서드 구현
  - DplyrNode와 DplyrOperation AST 노드 구조 완성
  - 표현식 파싱을 위한 Expr 열거형 및 관련 구조체 구현
  - 기본 파싱 오류 처리 로직 구현
  - _Requirements: 2.1, 2.2_

- [x] 6. 기본 dplyr 함수 파싱 구현
- [x] 6.1 select() 함수 파싱 구현
  - select 토큰 인식 및 컬럼 목록 파싱
  - ColumnExpr 구조체를 사용한 컬럼 표현식 처리
  - 별칭(alias) 지원 구현
  - _Requirements: 1.2_

- [x] 6.2 filter() 함수 파싱 구현
  - filter 토큰 인식 및 조건식 파싱
  - 비교 연산자 및 논리 연산자 처리
  - 복잡한 중첩 조건식 파싱 지원
  - _Requirements: 1.2_

- [x] 6.3 mutate() 함수 파싱 구현
  - mutate 토큰 인식 및 할당문 파싱
  - Assignment 구조체를 사용한 새 컬럼 생성 처리
  - 계산식 및 함수 호출 파싱 지원
  - _Requirements: 1.2_

- [x] 7. 추가 dplyr 함수 파싱 구현
- [x] 7.1 arrange() 함수 파싱 구현
  - arrange 토큰 인식 및 정렬 컬럼 파싱
  - OrderExpr 구조체를 사용한 정렬 방향 처리
  - desc() 함수 지원 구현
  - _Requirements: 1.2_

- [x] 7.2 group_by() 및 summarise() 함수 파싱 구현
  - group_by 토큰 인식 및 그룹핑 컬럼 파싱
  - summarise 토큰 인식 및 집계 함수 파싱
  - Aggregation 구조체를 사용한 집계 연산 처리
  - _Requirements: 1.2_

- [x] 8. 파이프라인 파싱 구현
  - 파이프 연산자(%>%) 체이닝 처리
  - 여러 dplyr 연산의 순차적 파싱
  - Pipeline AST 노드 생성 및 연산 순서 보존
  - _Requirements: 1.1, 2.1_

- [x] 9. Parser 단위 테스트 작성
  - 각 dplyr 함수별 파싱 테스트
  - 복잡한 파이프라인 파싱 테스트
  - 파싱 오류 케이스 테스트
  - AST 구조 검증 테스트
  - _Requirements: 6.1, 6.3_

- [x] 10. SQL 방언 트레이트 및 기본 구현체 작성
  - SqlDialect 트레이트 정의 및 기본 메서드 구현
  - PostgreSqlDialect, MySqlDialect, SqliteDialect 구조체 구현
  - 각 방언별 식별자 인용, 문자열 처리, 특화 문법 구현
  - DialectConfig 구조체를 사용한 방언 설정 관리
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [x] 11. SQL Generator 기본 구조 구현
  - SqlGenerator 구조체 및 생성 메서드 구현
  - AST 노드를 SQL로 변환하는 기본 프레임워크 구현
  - 방언별 SQL 생성을 위한 추상화 레이어 구현
  - _Requirements: 1.1, 3.1_

- [x] 12. 기본 SQL 생성 기능 구현
- [x] 12.1 SELECT 문 생성 구현
  - select 연산을 SQL SELECT 절로 변환
  - 컬럼 목록 및 별칭 처리
  - 방언별 식별자 인용 적용
  - _Requirements: 1.2_

- [x] 12.2 WHERE 절 생성 구현
  - filter 연산을 SQL WHERE 절로 변환
  - 비교 연산자 및 논리 연산자 변환
  - 복잡한 조건식 처리
  - _Requirements: 1.2_

- [x] 12.3 ORDER BY 절 생성 구현
  - arrange 연산을 SQL ORDER BY 절로 변환
  - 정렬 방향 처리 (ASC/DESC)
  - 다중 컬럼 정렬 지원
  - _Requirements: 1.2_

- [x] 13. 고급 SQL 생성 기능 구현
- [x] 13.1 GROUP BY 및 집계 함수 생성 구현
  - group_by 연산을 SQL GROUP BY 절로 변환
  - summarise 연산을 집계 함수로 변환
  - 방언별 집계 함수 차이 처리
  - _Requirements: 1.2_

- [x] 13.2 서브쿼리 및 복잡한 변환 구현
  - mutate 연산의 복잡한 계산식 처리
  - 필요시 서브쿼리 생성
  - 중첩된 파이프라인 처리
  - _Requirements: 1.1, 6.2_

- [x] 14. SQL Generator 단위 테스트 작성
  - 각 SQL 절별 생성 테스트
  - 방언별 SQL 생성 차이 테스트
  - 복잡한 쿼리 생성 테스트
  - 오류 케이스 테스트
  - _Requirements: 6.1, 6.2_

- [x] 15. 통합 Transpiler API 구현
  - Transpiler 구조체 및 공개 API 메서드 구현
  - transpile() 메서드로 전체 변환 파이프라인 연결
  - parse_dplyr() 및 generate_sql() 개별 기능 제공
  - 오류 타입 통합 및 Result 타입 반환
  - _Requirements: 5.1, 5.2_

- [x] 16. CLI 인터페이스 구현
  - clap을 사용한 명령줄 인수 파싱
  - 입력 파일 읽기 및 출력 파일 쓰기 기능
  - SQL 방언 선택 옵션 구현
  - 예쁜 출력(pretty-print) 옵션 구현
  - _Requirements: 4.1, 4.2, 4.3_

- [x] 17. CLI 오류 처리 및 도움말 구현
  - 파일 읽기/쓰기 오류 처리
  - 변환 오류 시 사용자 친화적 메시지 출력
  - 도움말 메시지 및 사용법 예시 구현
  - 적절한 종료 코드 반환
  - _Requirements: 4.3, 4.4_

- [x] 18. 통합 테스트 작성
  - 전체 변환 파이프라인 end-to-end 테스트
  - 다양한 dplyr 패턴의 SQL 변환 검증
  - 방언별 변환 결과 비교 테스트
  - CLI 인터페이스 통합 테스트
  - _Requirements: 6.1, 6.2_

- [x] 19. 성능 테스트 및 벤치마크 구현
  - criterion을 사용한 벤치마크 테스트 작성
  - 큰 AST 및 복잡한 쿼리 성능 측정
  - 메모리 사용량 및 처리 속도 최적화
  - _Requirements: 5.3_

- [x] 20. 문서화 및 예제 코드 작성
  - 공개 API에 대한 rustdoc 문서 작성
  - README.md 파일에 사용법 및 예제 추가
  - 각 방언별 사용 예시 코드 작성
  - 라이브러리 사용법 가이드 작성
  - _Requirements: 5.1, 4.1_