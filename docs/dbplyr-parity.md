# dbplyr 호환 범위와 제약 해소 결과

검토일: 2026-10-08. 비교 기준: dbplyr 2.6.0, dplyr 1.2.1.
기준 HEAD는 `d32307fc61086d0bbe2c3a7a744f7edcab12dcbf`이며, 아래 결과는 현재 미커밋 구현을 포함한다.

제안한 순서에 따라 의미 오류, 주요 동사·옵션·함수, 비등가 조인, tidyr와 실행 프로토콜을
추가했다. 정렬된 위치 slice, 실제 조인 관계 검사, 롤링 조인, 가중·복원 표본,
동적 피벗의 제약도 해소했다. 다만 전체 dbplyr API 또는 R 평가기와 완전히 동등하지는 않다.
구체적인 허용 문법과 방언 예외는 [지원 문서](dplyr-support.md)를 따른다.

## 구현 결과

| 영역 | 구현한 범위 | 조건과 남은 한계 |
| --- | --- | --- |
| 동사 옵션 | 빈·다중 `filter`, `filter_out`, `transmute`, `.by`, `.keep`, `.before/.after`, `.groups`, `relocate`, `rename_with` | `.by`는 비그룹 입력, 이름 변환은 `tolower/toupper` |
| 그룹·투영 | 계산 그룹 키, `.add`, 부분 `ungroup`, 그룹 열 변경 후 재그룹화, 계산 정렬, `.by_group` | 관측되지 않은 factor 수준은 메타데이터 필요 |
| 고유 행·개수 | 계산 `distinct`, `.keep_all`, 가중 count/tally, add_count/add_tally | count의 이름 있는 계산 그룹 키 등 일부 조합 미지원 |
| 여러 열 연산 | 선택식, 함수 목록, `~`·`function(v)` 람다, `cur_column`, `if_any/if_all` | 일반 R 평가, `.unpack`, 임의 glue 미지원 |
| 윈도우·함수 | 순서·프레임, 순위·누적 함수, 조건식·NULL 수정, median, sd/var, 다중 열 distinct 집계 | 이동 프레임의 distinct와 이식형 통계 미지원; SQLite는 math 함수 필요 |
| 조인 | 등가·부등호·구간·겹침·cross, 우측 파이프라인, NULL 매칭과 suffix | MySQL full join, 우측 파이프라인 안의 중첩 join 미지원 |
| 집합 연산 | union/all, intersect, setdiff, bind_queries와 없는 열의 NULL 보충 | intersect/setdiff의 ALL 미지원 |
| tidyr | pivot_longer/wider, fill 4방향, expand/complete, replace_na, dbplyr_uncount | wider는 하나의 이름·값 열; 전체 spec·옵션 조합 미지원 |
| 행 변경 결과 | rows_append/insert/update/patch/upsert/delete의 SELECT | 원본 DB 변경과 `in_place = TRUE` 미지원 |
| 바인딩·DB 표현식 | 타입 있는 상수·선택 목록, `.data/.env`, 안전한 함수 이름 전달, 표현식 `sql()` | `!!/!!!`, quosure, 전체 SQL 문법과 임의 R 함수 미지원 |
| 실행 | 검사 SQL과 결과 SQL, 실패 rollback, 피벗 키 탐색 | DB 드라이버는 호출자가 제공; 안정된 스냅샷 필요 |

`mutate(x = NULL)`은 열을 삭제하고, `mutate(x = NA)`는 열을 유지하며 SQL NULL을 쓴다.
두 형태는 `across()`에서도 구별한다. `.by/.keep/.groups` 등을 출력 열로 만드는 오류,
정렬이 lag 윈도우에 전달되지 않는 오류, rank·ntile·ifelse·nzchar의 NULL 처리 오류를 수정했다.
`na.rm = TRUE/FALSE`는 SQL 집계의 NULL 제거 의미를 사용한다. R의 NA 전파를 보증하지 않는다.

## 제약을 푼 방법

| 제약 | 현재 구현 | 유지하는 계약 |
| --- | --- | --- |
| 행 위치 기반 slice | 명시적 정렬 + `ROW_NUMBER`, 요청 위치 관계 또는 범위 필터 | 양수 위치의 순서·중복 보존, 음수 제외, 혼합 부호 거부. 결정성이 필요하면 총순서 필요 |
| 조인 관계·unmatched 검사 | 최종 매칭 조건으로 위반 SQL을 만들고 결과 전에 실행 | 검사와 결과가 같은 안정된 스냅샷. 매칭되지 않는 전역 중복은 관계 위반으로 취급하지 않음 |
| 롤링 조인 | 부등호 후보의 상관 MAX/MIN | 최근접 값의 모든 동률 보존 |
| 가중 비복원 표본 | `-ln(U) / weight` 순위 | 양수 후보, 유한·비음수 가중치와 양수 합 검증, 가중치 정규화로 합계 오버플로 완화 |
| 복원 표본 | 추출 번호별 난수 + 누적 가중치 구간 | 추출별 난수를 한 번 계산해 재사용, 0 가중치 제외, 빈 입력은 빈 결과 |
| 동적 pivot_wider | 같은 스냅샷에서 키 탐색 후 AST에 주입하고 실행 | 명시적 keys도 지원. 빈 키 도메인은 ID 고유 행 반환 |

단일 정수 slice 범위는 열거하지 않는다. `c()` 안의 범위는 현재 100001개 위치로 제한한다.
`multiple = "first/last/any"`는 후보 순서 계약이 없어 거부한다. `relationship`의 네 값과
`unmatched = "error"`는 실행 계획으로 지원한다. 검사 입력에 난수·휘발성 함수가 있으면
재평가 차이를 막기 위해 거부한다. CTE만으로 단일 평가를 보장한다고 가정하지 않는다.

## API 사용 경계

- `transpile_with_schemas()`는 검사 없는 단일 SELECT를 반환한다.
- `transpile_with_bindings()`는 JSON 상수·목록을 AST 리터럴로 바인딩한다.
- `plan_with_schemas()` 또는 CLI `--schema ... --execution-plan`은 검사와 결과를 반환한다.
- `execute(plan, adapter)`는 스냅샷 시작 → 검사 → 결과 버퍼링 → commit을 수행한다.
  검사·조회·commit 실패 시 rollback하며, rollback도 실패하면 두 오류를 보존한다.
- `execute_with_pivot_discovery()`는 키 탐색부터 결과까지 같은 스냅샷에서 실행한다.

`SnapshotExecutor`와 `PivotExecutor`의 실제 DB 구현은 호출자가 제공한다. 같은 연결과
트랜잭션만으로 충분하지 않다. 문장마다 읽기 상태가 바뀌는 READ COMMITTED는 계약을
충족하지 않는다. C API와 DuckDB SELECT 확장은 실행 계획의 검사를 수행하지 않으며,
그 검사가 필요한 입력을 순수 컴파일 성공으로 처리하지 않는다.

`sql("...")`은 기존 파서로 산술·함수 호출을 파싱하고 바인딩한다. SQL CASE, 서브쿼리,
문장·임의 문자열 조각을 삽입하는 경로가 아니다. 알 수 없는 안전한 함수 이름의 전달은
DB에 그 함수가 설치돼 있다는 보증이 아니다.

## 측정 결과

초기 조사와 동일한 101개 파이프라인을 다시 평가했다. 부족한 기능을 찾도록 선택한
프로브이므로 이 숫자를 전체 dbplyr 지원률로 해석하면 안 된다.

| 방언 | dbplyr SQL 생성 | libdplyr 초기 | libdplyr 현재 | 양쪽 생성 | dbplyr만 생성 | libdplyr만 생성 | 양쪽 거부 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| SQLite | 84 | 30 | 90 | 82 | 2 | 8 | 9 |
| PostgreSQL | 84 | 30 | 90 | 82 | 2 | 8 | 9 |
| MySQL | 83 | 30 | 90 | 81 | 2 | 9 | 9 |
| DuckDB | 비교하지 않음 | 31 | 90 | — | — | — | — |

여기서 순수 CLI가 거부한 `env_literal`은 타입 바인딩 API가 필요하며, `rows_patch`는
실행 계획이 필요하다. 가중 표본과 관계 검사도 순수 CLI 거부를 기능 미지원으로 세면
안 된다. 프로브 TSV의 `required_api`가 이를 구별하며 초기 결과도 별도 열에 보존했다.

SQLite에서 양쪽이 생성한 82개 중 79개를 실제 비교했다. 열 순서·행 다중집합 또는
표본 개수 기준으로 72개가 일치하고 7개가 달랐다. 나머지 3개는 기준 dbplyr SQL에
`STDEV`, 윈도우 DISTINCT 또는 사용자 DB 함수가 필요해 실행되지 않았다.
이는 SQL 생성 성공과 엔진 실행 가능성의 차이다.

실행 엔진: SQLite 3.53.4, DuckDB 1.5.5, PostgreSQL 17.11, MySQL 8.4.11.

현재 후보의 추가 검증:

- `cargo test --workspace --no-fail-fast`: 1161개 통과, 1개 기존 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings`와 fmt 검사 통과.
- SQLite·DuckDB·PostgreSQL·MySQL: 62개 결과 사례씩, **248/248** 통과.
- 같은 네 엔진: 관계·가중치·uncount·행 변경 위반 검사 **32개** 통과.
- DuckDB 확장: **85개** 통과. 테이블 함수와 embedded pipeline 진입점 포함.
- 기존 SQLite relational/followup 실행 회귀 통과.
- SQLite 두 연결의 동시 변경: 관계 검사와 피벗 탐색 각각 같은 스냅샷을 유지하는 **2개** 통과.
- Rust 실행·피벗 테스트: 실패 시 결과 미공개, commit/rollback 순서와 오류 보존 확인.

실제 서버의 결과·검사 SQL은 네 엔진에서 검증했다. 네 엔진 모두에 대한 드라이버 및
동시 변경 통합은 인증하지 않았다. SQLite의 동시 변경 검증과 Rust 프로토콜 검증은
호출자 DB 어댑터의 격리 수준 설정을 대신하지 않는다. 난수 분포, 시드 재현성,
모든 타입·collation과 모든 옵션 조합도 인증하지 않았다.

재현 명령은 `python3 tests/parity_execution.py`와 `python3 tests/snapshot_execution.py`다.
PostgreSQL 17·MySQL 8.4의 임시 테스트 컨테이너를 준비하면 첫 명령에 `--containers`를
추가한다. fixture는 매번 테스트 컨테이너의 `data/other/keys`를 다시 만든다.

## 의도적으로 남긴 의미 차이

아래는 고정 버전 dbplyr의 SQLite 결과와 다른 정책이다. 문법을 받아들인다는 사실과
결과 의미가 완전히 같다는 주장은 구별한다.

| 사례 | libdplyr 의미 | 이번 dbplyr 결과 |
| --- | --- | --- |
| 무그룹 상수 summarise | 입력 개수와 무관한 1행 | 입력과 같은 6행 |
| n_distinct 기본 | NULL 포함 | NULL 제외 |
| ntile(x, n) | NULL은 결과 NULL이며 순위에서 제외 | NULL도 버킷에 참여 |
| 정수 나눗셈 | 실수 나눗셈 | SQLite 정수 나눗셈 |
| 음수 %% | R의 floor 기반 나머지 | SQL 나머지의 부호 |
| as.Date | SQLite DATE 함수의 텍스트·NULL 결과 | SQLite CAST의 숫자 결과 |
| complete의 NULL 도메인 | NULL끼리 매칭해 원래 행 보존 | SQL equality에서 미매칭 행 추가 |

## 남은 범위

전체 R 평가기, `!!/!!!`, `reframe/rowwise/nest_join`, 같은 summarise 내 별칭 재사용,
그룹 키 덮어쓰기 요약, `.drop = FALSE`, 모든 tidyr 옵션과 추가 DB 방언은 남아 있다.
이동 프레임 distinct·이식형 통계, ALL intersect/except, 정렬 없는 first/last 후보 선택도
남아 있다. 표현식·SELECT 단계의 64 한도는 자원·안전 경계로 유지한다.

`collect/pull/compute/copy_to`의 데이터 이동·결과 수명 관리, DB 드라이버, DML과 원본
테이블 변경은 이번 구현에 포함하지 않았다. SQL 등가식이 없는 임의 R 함수에는 UDF,
R 호스트 연결 또는 로컬 실행이 필요하다. 이 부분은 단일 SELECT 컴파일로 해결되지 않는다.

## 근거

공식 [API 목록](https://dbplyr.tidyverse.org/reference/index.html),
[함수 번역](https://dbplyr.tidyverse.org/articles/translation-function.html),
[동사 번역](https://dbplyr.tidyverse.org/articles/translation-verb.html)과
[v2.6.0 소스](https://github.com/tidyverse/dbplyr/tree/v2.6.0)를 비교 기준으로 사용했다.
일반 문서와 허용 옵션이 다르면 고정 소스와 실행 결과를 우선했다.
독립 GPT-6 Pro 검토의 스냅샷·휘발성 입력 권고를 반영했으며, 검토 의견은 실제 검증을
대신하지 않는다.

[프로브 목록](dbplyr-parity-probes.tsv)은 초기·현재 결과 404개를 보존한다.
[함수 목록](dbplyr-function-inventory.tsv)의 670개 등록 항목·204개 이름은 고정 dbplyr의
scalar/aggregate/window 환경 목록이며 libdplyr 지원 보증 목록은 아니다.
원래 비교 fixture는 `id/grp/x/y`의 6행이며 프로브에 보존한 pipeline을 같은 스키마로
평가했다. R 기준은 `lazy_frame`과 `sql_render`, 외부 상수는 `threshold <- 3`이다.
고정 태그 아카이브 SHA256은
`b58420d58e1284844567b96309f4e4f454c830af58f264c961767a3eab8acbbf`다.

구현 위치는 `src/relational/`, `src/execution.rs`, `src/pivot_execution.rs`, 파서·방언 함수
번역과 공개 `Transpiler` API다. 기존 관계 AST와 단계별 SELECT를 재사용했다.
