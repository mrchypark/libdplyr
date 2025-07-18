# Requirements Document

## Introduction

libdplyr CLI는 stdin/stdout을 통해 dplyr 문법을 실시간으로 검증하고 SQL로 변환하는 명령줄 도구입니다. 이 기능은 파이프라인 처리, 스크립트 자동화, 그리고 대화형 사용을 지원하여 개발자들이 dplyr 코드를 효율적으로 SQL로 변환할 수 있게 합니다.

## Requirements

### Requirement 1

**User Story:** 개발자로서, stdin으로 dplyr 코드를 입력하고 stdout으로 변환된 SQL을 받고 싶습니다. 그래야 파이프라인이나 스크립트에서 쉽게 사용할 수 있습니다.

#### Acceptance Criteria

1. WHEN 사용자가 stdin으로 dplyr 코드를 입력하면 THEN 시스템은 해당 코드를 읽어야 합니다
2. WHEN 유효한 dplyr 코드가 입력되면 THEN 시스템은 변환된 SQL을 stdout으로 출력해야 합니다
3. WHEN EOF(Ctrl+D)가 입력되면 THEN 시스템은 처리를 완료하고 종료해야 합니다
4. WHEN 빈 입력이 제공되면 THEN 시스템은 오류 없이 빈 출력을 반환해야 합니다

### Requirement 2

**User Story:** 시스템 관리자로서, CLI 도구가 다양한 SQL 방언을 지원하기를 원합니다. 그래야 다른 데이터베이스 환경에서 유연하게 사용할 수 있습니다.

#### Acceptance Criteria

1. WHEN 사용자가 `--dialect` 옵션으로 PostgreSQL을 지정하면 THEN 시스템은 PostgreSQL 호환 SQL을 생성해야 합니다
2. WHEN 사용자가 `--dialect` 옵션으로 MySQL을 지정하면 THEN 시스템은 MySQL 호환 SQL을 생성해야 합니다
3. WHEN 사용자가 `--dialect` 옵션으로 SQLite를 지정하면 THEN 시스템은 SQLite 호환 SQL을 생성해야 합니다
4. WHEN 사용자가 `--dialect` 옵션으로 DuckDB를 지정하면 THEN 시스템은 DuckDB 호환 SQL을 생성해야 합니다
5. WHEN 방언이 지정되지 않으면 THEN 시스템은 기본값으로 PostgreSQL을 사용해야 합니다

### Requirement 3

**User Story:** 개발자로서, dplyr 코드의 문법 검증 기능을 원합니다. 그래야 변환 전에 코드의 유효성을 확인할 수 있습니다.

#### Acceptance Criteria

1. WHEN 사용자가 `--validate-only` 옵션을 사용하면 THEN 시스템은 SQL 변환 없이 문법 검증만 수행해야 합니다
2. WHEN 문법 검증이 성공하면 THEN 시스템은 "Valid dplyr syntax" 메시지를 출력하고 exit code 0을 반환해야 합니다
3. WHEN 문법 검증이 실패하면 THEN 시스템은 구체적인 오류 메시지를 stderr로 출력하고 exit code 1을 반환해야 합니다
4. WHEN 검증 모드에서 유효한 코드가 입력되면 THEN 시스템은 AST 구조 정보를 선택적으로 출력할 수 있어야 합니다

### Requirement 4

**User Story:** 스크립트 작성자로서, CLI 도구가 적절한 exit code를 반환하기를 원합니다. 그래야 스크립트에서 성공/실패를 정확히 판단할 수 있습니다.

#### Acceptance Criteria

1. WHEN 변환이 성공적으로 완료되면 THEN 시스템은 exit code 0을 반환해야 합니다
2. WHEN 입력 코드에 문법 오류가 있으면 THEN 시스템은 exit code 1을 반환해야 합니다
3. WHEN 지원되지 않는 방언이 지정되면 THEN 시스템은 exit code 2를 반환해야 합니다
4. WHEN 시스템 오류(메모리 부족 등)가 발생하면 THEN 시스템은 exit code 3을 반환해야 합니다
5. WHEN 잘못된 CLI 옵션이 제공되면 THEN 시스템은 도움말을 출력하고 exit code 2를 반환해야 합니다

### Requirement 5

**User Story:** 개발자로서, 출력 형식을 제어할 수 있는 옵션이 필요합니다. 그래야 다양한 용도에 맞게 결과를 활용할 수 있습니다.

#### Acceptance Criteria

1. WHEN 사용자가 `--pretty` 옵션을 사용하면 THEN 시스템은 들여쓰기와 줄바꿈이 적용된 가독성 좋은 SQL을 출력해야 합니다
2. WHEN 사용자가 `--compact` 옵션을 사용하면 THEN 시스템은 공백을 최소화한 한 줄 SQL을 출력해야 합니다
3. WHEN 출력 형식 옵션이 지정되지 않으면 THEN 시스템은 기본 형식(적당한 가독성)으로 출력해야 합니다
4. WHEN 사용자가 `--json` 옵션을 사용하면 THEN 시스템은 SQL과 메타데이터를 JSON 형식으로 출력해야 합니다

### Requirement 6

**User Story:** 디버깅을 하는 개발자로서, 상세한 오류 정보와 진단 메시지가 필요합니다. 그래야 문제를 빠르게 해결할 수 있습니다.

#### Acceptance Criteria

1. WHEN 파싱 오류가 발생하면 THEN 시스템은 오류 위치(행, 열)를 정확히 표시해야 합니다
2. WHEN 사용자가 `--verbose` 옵션을 사용하면 THEN 시스템은 처리 단계별 상세 정보를 stderr로 출력해야 합니다
3. WHEN 변환 과정에서 경고가 발생하면 THEN 시스템은 경고 메시지를 stderr로 출력하되 처리는 계속해야 합니다
4. WHEN 사용자가 `--debug` 옵션을 사용하면 THEN 시스템은 AST 구조와 변환 과정을 상세히 출력해야 합니다
5. IF 오류 메시지가 출력되면 THEN 가능한 해결 방법이나 힌트를 포함해야 합니다

### Requirement 7

**User Story:** 시스템 통합 담당자로서, CLI 도구가 Unix 파이프라인과 잘 통합되기를 원합니다. 그래야 다른 도구들과 함께 사용할 수 있습니다.

#### Acceptance Criteria

1. WHEN stdin이 파이프로 연결되면 THEN 시스템은 자동으로 파이프 모드로 동작해야 합니다
2. WHEN stdout이 파이프로 연결되면 THEN 시스템은 진행 상황 메시지를 stderr로만 출력해야 합니다
3. WHEN 시그널(SIGINT, SIGTERM)을 받으면 THEN 시스템은 정상적으로 종료해야 합니다
4. WHEN 대용량 입력이 제공되면 THEN 시스템은 메모리 효율적으로 처리해야 합니다
5. IF 파이프가 끊어지면 THEN 시스템은 SIGPIPE를 적절히 처리해야 합니다

### Requirement 8

**User Story:** 시스템 관리자로서, 다양한 플랫폼에서 libdplyr을 쉽게 설치하고 사용하고 싶습니다. 그래야 개발 환경에 관계없이 일관된 도구를 사용할 수 있습니다.

#### Acceptance Criteria

1. WHEN 새로운 버전이 태그(v*)로 릴리즈되면 THEN 시스템은 자동으로 크로스 플랫폼 바이너리를 빌드해야 합니다
2. WHEN 바이너리가 빌드되면 THEN 시스템은 Linux(x86_64, ARM64), macOS(Intel, Apple Silicon), Windows(x86_64) 바이너리를 생성해야 합니다
3. WHEN 릴리즈가 생성되면 THEN 시스템은 자동으로 릴리즈 노트와 함께 바이너리를 GitHub Releases에 업로드해야 합니다
4. WHEN 사용자가 install.sh 스크립트를 실행하면 THEN 시스템은 최신 버전의 바이너리를 자동으로 다운로드하고 설치해야 합니다
5. WHEN 사용자가 특정 버전을 지정하면 THEN 시스템은 해당 버전의 바이너리를 다운로드하고 설치해야 합니다

### Requirement 9

**User Story:** 개발자로서, libdplyr을 빠르고 간편하게 설치할 수 있는 방법이 필요합니다. 그래야 복잡한 빌드 과정 없이 바로 사용할 수 있습니다.

#### Acceptance Criteria

1. WHEN 사용자가 `curl -sSL https://raw.githubusercontent.com/libdplyr/libdplyr/main/install.sh | sh`를 실행하면 THEN 시스템은 최신 버전을 설치해야 합니다
2. WHEN 사용자가 `LIBDPLYR_VERSION=v1.0.0 ./install.sh`를 실행하면 THEN 시스템은 지정된 버전을 설치해야 합니다
3. WHEN 설치가 완료되면 THEN 시스템은 바이너리를 `/usr/local/bin/libdplyr`에 설치하고 실행 권한을 부여해야 합니다
4. WHEN 설치 중 오류가 발생하면 THEN 시스템은 명확한 오류 메시지와 해결 방법을 제공해야 합니다
5. IF 사용자가 관리자 권한이 없으면 THEN 시스템은 `~/.local/bin` 디렉토리에 설치하는 옵션을 제공해야 합니다