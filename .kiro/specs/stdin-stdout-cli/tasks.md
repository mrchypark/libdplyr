# Implementation Plan

- [x] 1. CLI 인수 파서 확장 및 새로운 옵션 추가
  - 기존 `src/cli.rs`의 `CliArgs` 구조체에 새로운 필드들 추가
  - `--validate-only`, `--verbose`, `--debug`, `--compact`, `--json` 옵션 구현
  - clap 설정 업데이트하여 새로운 명령줄 옵션들 지원
  - _Requirements: 1.1, 2.1, 3.1, 5.1, 6.2_

- [x] 2. Stdin 입력 처리 모듈 구현
  - `src/cli/stdin_reader.rs` 모듈 생성
  - `StdinReader` 구조체와 전체 입력 읽기 기능 구현
  - 파이프 연결 감지 기능 (`is_piped`) 구현
  - 빈 입력 및 EOF 처리 로직 구현
  - _Requirements: 1.1, 1.2, 1.3, 7.1_

- [x] 3. 출력 형식 처리 모듈 구현
  - `src/cli/output_formatter.rs` 모듈 생성
  - `OutputFormatter` 구조체와 기본 형식 지원 구현
  - Pretty 형식 (기존 `format_sql` 함수 활용) 구현
  - Compact 형식 (공백 최소화) 구현
  - _Requirements: 5.1, 5.2, 5.3_

- [x] 4. JSON 출력 형식 및 메타데이터 지원 구현
  - JSON 출력 형식 구현 (`serde_json` 사용)
  - `TranspileMetadata` 구조체 정의 및 메타데이터 수집
  - 타임스탬프, 방언 정보, 처리 통계 포함
  - _Requirements: 5.4_

- [x] 5. 문법 검증 전용 모듈 구현
  - `src/cli/validator.rs` 모듈 생성
  - `DplyrValidator` 구조체와 검증 로직 구현
  - `ValidationResult` 열거형과 성공/실패 처리
  - 오류 시 제안사항 생성 기능 구현
  - _Requirements: 3.1, 3.2, 3.3, 3.4_

- [x] 6. 향상된 오류 처리 및 Exit Code 관리 구현
  - `src/cli/error_handler.rs` 모듈 생성
  - `ErrorHandler` 구조체와 오류별 처리 로직 구현
  - Exit code 상수 정의 및 적절한 코드 반환 구현
  - 상세한 오류 메시지와 힌트 제공 기능 구현
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 6.1, 6.5_

- [x] 7. 처리 파이프라인 통합 및 모드 관리 구현
  - `CliConfig`와 `CliMode` 열거형 구현
  - `ProcessingPipeline` 구조체로 전체 처리 흐름 통합
  - 파일 모드, 텍스트 모드, Stdin 모드 분기 처리
  - 검증 전용 모드와 변환 모드 분기 구현
  - _Requirements: 1.1, 2.1, 3.1_

- [x] 8. 기존 CLI 함수 업데이트 및 통합
  - `run_cli()` 함수를 새로운 파이프라인 구조로 리팩토링
  - 기존 파일 기반 처리와 새로운 stdin 처리 통합
  - 하위 호환성 유지하면서 새로운 기능 추가
  - _Requirements: 1.1, 1.4_

- [x] 9. Verbose 및 Debug 출력 기능 구현
  - 처리 단계별 상세 정보 출력 (stderr 사용)
  - Debug 모드에서 AST 구조 출력 기능
  - 진행 상황 메시지와 경고 메시지 처리
  - 파이프 모드에서 stderr로만 진단 메시지 출력
  - _Requirements: 6.2, 6.3, 6.4, 7.2_

- [x] 10. Unix 파이프라인 통합 및 시그널 처리 구현
  - 파이프 연결 자동 감지 및 모드 전환
  - SIGINT, SIGTERM 시그널 처리 구현
  - SIGPIPE 처리 및 정상 종료 로직
  - 대용량 입력에 대한 메모리 효율적 처리
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5_

- [x] 11. 단위 테스트 구현
  - `StdinReader` 모듈 테스트 작성
  - `OutputFormatter` 각 형식별 테스트 작성
  - `DplyrValidator` 검증 로직 테스트 작성
  - `ErrorHandler` 오류 처리 테스트 작성
  - _Requirements: 모든 요구사항의 검증_

- [x] 12. 통합 테스트 구현
  - stdin/stdout 기본 동작 통합 테스트
  - 검증 전용 모드 통합 테스트
  - JSON 출력 형식 통합 테스트
  - 다양한 CLI 옵션 조합 테스트
  - Exit code 검증 테스트
  - _Requirements: 모든 요구사항의 end-to-end 검증_

- [x] 13. 성능 벤치마크 및 최적화
  - stdin 처리 성능 벤치마크 작성
  - 메모리 사용량 측정 및 최적화
  - 대용량 입력 처리 성능 테스트
  - JSON 직렬화 성능 최적화
  - _Requirements: 7.4_

- [x] 14. 문서 업데이트 및 사용 예시 추가
  - README.md에 stdin/stdout 사용법 추가
  - CLI 도움말 메시지 모두 영어로
  - 파이프라인 사용 예시 및 베스트 프랙티스 문서화
  - 오류 해결 가이드 업데이트
  - _Requirements: 6.5_

- [x] 15. 크로스 플랫폼 호환성 테스트 및 수정
  - Unix 시스템에서 파이프 감지 테스트
  - Windows 호환성 확인 및 조건부 컴파일 적용
  - 시그널 처리의 플랫폼별 구현 테스트
  - CI/CD 파이프라인에서 다중 플랫폼 테스트
  - _Requirements: 7.1, 7.3_

- [x] 16. GitHub Actions 릴리즈 워크플로우 구현
  - `.github/workflows/release.yml` 파일 생성
  - 크로스 플랫폼 빌드 매트릭스 설정 (Linux x86_64/ARM64, macOS Intel/Apple Silicon, Windows x86_64)
  - 바이너리 빌드, 스트립, 아티팩트 업로드 단계 구현
  - 릴리즈 노트 자동 생성 및 GitHub Releases 업로드
  - _Requirements: 8.1, 8.2, 8.3_

- [x] 17. 설치 스크립트 구현
  - `install.sh` 스크립트 생성
  - 플랫폼 자동 감지 기능 (OS 및 아키텍처)
  - 최신 버전 및 특정 버전 다운로드 지원
  - 권한 처리 및 폴백 설치 디렉토리 지원
  - _Requirements: 8.4, 8.5, 9.1, 9.2, 9.3, 9.5_

- [x] 18. 설치 스크립트 오류 처리 및 사용자 경험 개선
  - 네트워크 오류, 권한 오류, 플랫폼 미지원 등 오류 상황 처리
  - 상세한 오류 메시지와 해결 방법 제공
  - 설치 진행 상황 표시 및 성공/실패 피드백
  - PATH 설정 안내 및 설치 확인 기능
  - _Requirements: 9.4, 9.5_

- [ ] 19. README.md 설치 및 사용법 문서 업데이트
  - 자동 설치 방법 (curl 명령어) 추가
  - 수동 설치 방법 및 지원 플랫폼 정보 추가
  - 기본 사용법 및 고급 옵션 예시 추가
  - 파이프라인 사용 예시 및 베스트 프랙티스 문서화
  - _Requirements: 8.3, 9.1, 9.2_

- [ ] 20. 릴리즈 프로세스 테스트 및 검증
  - 테스트 태그로 릴리즈 워크플로우 동작 확인
  - 각 플랫폼별 바이너리 다운로드 및 실행 테스트
  - 설치 스크립트의 다양한 환경에서 동작 테스트
  - 버전 지정 설치 및 최신 버전 설치 테스트
  - _Requirements: 8.1, 8.2, 8.3, 9.1, 9.2_