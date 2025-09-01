# 구현 계획

이 구현 계획은 requirements.md의 R1-R10 요구사항과 design.md의 아키텍처를 기반으로 libdplyr를 DuckDB 확장으로 변환하는 단계별 작업을 정의합니다.

## 1단계: 프로젝트 구조 및 기본 설정

- [x] 1. 프로젝트 구조 생성
  - libdplyr_c/ 디렉토리에 새로운 Rust 크레이트 생성
  - extension/ 디렉토리에 DuckDB 확장 C++ 소스 구성
  - 루트에 CMakeLists.txt와 extension_config.cmake 설정
  - _요구사항: R4-AC1, R8-AC1_

- [x] 2. Rust 크레이트 기본 설정
  - [x] 2.1 Cargo.toml 설정
    - crate-type을 ["staticlib", "cdylib"]로 설정
    - 필요한 의존성 추가 (libdplyr, libc, panic 관련)
    - panic = "unwind" 설정으로 FFI 경계에서 panic 처리 준비
    - _요구사항: R3-AC1, R9-AC1_

  - [x] 2.2 기본 모듈 구조 생성
    - src/lib.rs에 FFI 함수 선언부 작성
    - src/error.rs에 에러 타입 정의
    - src/cache.rs에 캐싱 로직 구현 준비
    - _요구사항: R6-AC1, R9-AC2_

## 2단계: C-API 핵심 구현

- [x] 3. C-호환 데이터 구조 정의
  - [x] 3.1 DplyrOptions 구조체 구현
    - #[repr(C)]로 C 호환성 확보
    - strict_mode, preserve_comments, debug_mode, max_input_length 필드
    - 기본값 생성 함수 구현
    - _요구사항: R3-AC1, R9-AC2, R10-AC1_

  - [x] 3.2 에러 코드 체계 구현
    - 부록 C 에러 코드 체계에 따른 상수 정의
    - TranspileError enum을 C 문자열로 변환하는 로직
    - 에러 코드, 위치, 제안사항 포함한 포맷팅
    - _요구사항: R1-AC3, R2-AC3_

- [x] 4. 핵심 FFI 함수 구현
  - [x] 4.1 dplyr_compile 함수 구현
    - panic::catch_unwind로 panic 안전성 확보
    - 입력 검증 (NULL 포인터, UTF-8 인코딩, 길이 제한)
    - libdplyr Transpiler와 통합하여 실제 변환 수행
    - out_sql, out_error 포인터를 통한 결과 반환
    - _요구사항: R3-AC2, R9-AC1, R9-AC2_

  - [x] 4.2 메모리 관리 함수 구현
    - dplyr_free_string으로 할당된 문자열 해제
    - CString::from_raw을 사용한 안전한 메모리 정리
    - 이중 해제 방지 로직
    - _요구사항: R3-AC3, R6-AC3_

  - [x] 4.3 유틸리티 함수 구현
    - dplyr_version 함수로 버전 정보 반환
    - 정적 문자열로 버전 관리
    - _요구사항: R8-AC1_

- [x] 5. 캐싱 시스템 구현
  - [x] 5.1 단순 캐시 구현
    - thread_local 기반 요청 범위 캐시
    - LRU 정책으로 캐시 크기 제한 (100개)
    - 캐시 키 생성 (code + options 해시)
    - _요구사항: R6-AC1_

  - [x] 5.2 캐시 메타데이터 노출
    - 캐시 히트율 및 크기 정보 수집
    - 디버그 모드에서 캐시 통계 로깅
    - _요구사항: R10-AC2_

## 3단계: DuckDB 확장 구현

- [x] 6. C 헤더 파일 작성
  - dplyr_extension.h에 모든 FFI 함수 선언
  - stdint.h, stdbool.h 포함으로 타입 안정성 확보
  - C++ 호환성을 위한 extern "C" 블록
  - 상세한 문서 주석으로 사용법 설명
  - _요구사항: R8-AC2_

- [x] 7. DuckDB 파서 확장 구현
  - [x] 7.1 DplyrParserExtension 클래스 구현
    - DPLYR 키워드 감지 및 문자열 리터럴 추출
    - dplyr_compile 호출하여 SQL 변환
    - 변환된 SQL을 DuckDB 파서에 재주입
    - _요구사항: R5-AC1, R2-AC1_

  - [x] 7.2 키워드 처리 로직 구현
    - DplyrKeywordProcessor 클래스로 키워드 검증
    - 리터럴 사전 유효성 체크 (선택적)
    - 파싱 실패 시 명확한 에러 반환
    - _요구사항: R5-AC2, R5-AC3_

- [x] 8. 확장 등록 및 로딩 완성
  - 파서 확장 등록 (테이블 함수 제거)
  - DuckDB 확장 템플릿 패턴 준수
  - 시스템 검증 및 디버그 로깅
  - _요구사항: R2-AC1, R4-AC2_

## 4단계: 에러 처리 및 진단 기능

- [x] 9. 에러 처리 시스템 구현
  - [x] 9.1 DplyrErrorHandler 클래스 구현
    - 에러 코드별 적절한 DuckDB 예외 타입 선택
    - 크래시 방지를 위한 안전한 에러 처리
    - 에러 메시지 포맷팅 및 컨텍스트 정보 포함
    - _요구사항: R7-AC3, R1-AC3_

  - [x] 9.2 디버그 로깅 시스템 구현
    - 환경변수 및 세션 옵션으로 디버그 모드 토글
    - DuckDB 로깅 시스템과 통합
    - 타임스탬프 및 카테고리별 로그 기록
    - _요구사항: R10-AC1_

- [x] 10. 입력 검증 및 보안 강화
  - [x] 10.1 입력 검증 로직 구현
    - NULL 포인터, UTF-8 인코딩, 길이 제한 검사
    - DoS 방지를 위한 처리 시간 제한
    - 악성 입력에 대한 안전한 처리
    - _요구사항: R9-AC2_

  - [x] 10.2 스레드 안전성 확보
    - thread_local 캐시로 동시성 문제 회피
    - FFI 함수의 재진입 안전성 확보
    - 공유 상태 최소화
    - _요구사항: R9-AC3_

## 5단계: 빌드 시스템 및 테스트

- [-] 11. CMake 빌드 시스템 구성
  - [x] 11.1 CMakeLists.txt 작성
    - Corrosion을 사용한 Rust 정적 라이브러리 통합
    - DuckDB 확장 템플릿의 build_loadable_extension 사용
    - 플랫폼별 링킹 설정 (Windows, macOS, Linux)
    - _요구사항: R4-AC1, R3-AC1_

  - [x] 11.2 extension_config.cmake 설정
    - 확장 메타데이터 및 의존성 정의
    - 빌드 옵션 및 컴파일러 플래그 설정
    - _요구사항: R8-AC1_

- [ ] 12. 테스트 구현
  - [x] 12.1 Rust 단위 테스트 작성
    - FFI 함수의 panic 안전성 테스트
    - 메모리 관리 및 에러 처리 테스트
    - 캐싱 로직 및 성능 테스트
    - _요구사항: R9-AC1, R6-AC1_

  - [x] 12.2 C++ 통합 테스트 작성
    - DuckDB 확장 로딩 및 기능 테스트
    - 표준 SQL과의 혼용 테스트
    - 크래시 방지 및 에러 처리 테스트
    - _요구사항: R7-AC1, R7-AC3_

  - [x] 12.3 스모크 테스트 작성
    - tests/smoke.sql에 기본 기능 테스트 쿼리 작성
    - DPLYR 키워드 기반 구문 테스트
    - 최소 연산 집합 (select, filter, mutate 등) 테스트
    - _요구사항: R4-AC2, R1-AC2_

## 6단계: CI/CD 및 배포

- [ ] 13. GitHub Actions 워크플로우 구성
  - [x] 13.1 빌드 매트릭스 설정
    - 지원 플랫폼별 빌드 (Linux x86_64, macOS x86_64/arm64, Windows x86_64)
    - Rust 및 C++ 컴포넌트 빌드 검증
    - 스모크 테스트 자동 실행
    - _요구사항: R4-AC1, R4-AC2_

  - [x] 13.2 코드 품질 검사
    - 코드 커버리지 측정 (핵심 경로 70% 이상)
    - Rust clippy 및 C++ 정적 분석
    - 메모리 누수 검사 (valgrind 등)
    - _요구사항: R7-AC4_

- [ ] 14. 릴리스 자동화
  - [x] 14.1 아티팩트 패키징
    - 플랫폼별 확장 바이너리 패키징
    - 버전 정보 및 호환성 메타데이터 포함
    - 압축 및 체크섬 생성
    - _요구사항: R4-AC3_

  - [x] 14.2 GitHub Releases 배포
    - 태그 기반 자동 릴리스 생성
    - 릴리스 노트에 호환성 정보 및 변경사항 포함
    - Community repo 등록 준비 (선택적)
    - _요구사항: R4-AC3, R8-AC3_

## 7단계: 문서화 및 최적화

- [ ] 16. 성능 최적화 및 검증
  - [x] 16.1 성능 벤치마크 구현
    - 단순/복잡 파이프라인 성능 측정
    - P95 목표 달성 검증 (2ms/15ms)
    - 확장 로딩 시간 측정 (50ms 미만)
    - _요구사항: R6-AC1, R6-AC2_

  - [ ] 16.2 메모리 안정성 검증
    - 반복 실행 시 메모리 증가율 측정 (10% 미만)
    - 메모리 누수 검사 및 수정
    - 장시간 실행 안정성 테스트
    - _요구사항: R6-AC3_

## 구현 순서 권장사항

1. **1-2단계 (기반 구조)**: 프로젝트 구조와 기본 C-API 구현
2. **3-4단계 (핵심 기능)**: DuckDB 통합 및 에러 처리
3. **5단계 (빌드/테스트)**: 빌드 시스템 및 기본 테스트
4. **6-7단계 (배포/최적화)**: CI/CD 및 성능 최적화

각 단계는 이전 단계의 완료를 전제로 하며, 단계 내 작업들은 병렬로 진행 가능합니다.