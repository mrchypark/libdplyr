# DuckDB Community Extensions 제출 가이드

## 현재 기준

- 제출 대상 저장소: `duckdb/community-extensions`
- 제출 파일: `extensions/dplyr/description.yml` 하나
- 현재 준비 파일: [community-pr/description.yml](/Users/cypark/Documents/project/libdplyr/community-pr/description.yml)
- 현재 최신 stable DuckDB: `1.5.2`

## libdplyr에 중요한 해석

`libdplyr`는 DuckDB C++ extension API를 직접 쓰는 확장입니다. 이 경로는 DuckDB 공식 문서 기준으로 **unstable API extension** 범주에 가깝습니다. 따라서:

- 저장소 차원에서 `1.4.0`과 `1.5.2`를 둘 다 CI로 검증하는 것은 가능
- 하지만 community-extensions 배포 모델에서 "하나의 바이너리로 1.4.x와 1.5.x 동시 지원"을 주장하는 것은 부정확
- community-extensions 제출은 **현재 최신 stable 기준 배포**로 이해하는 것이 맞음
- `1.4.0` 지원은 배포 보장이라기보다 **repo-level source compatibility check**로 표현하는 편이 정확

## 제출 전에 맞춰야 하는 것

### 1. descriptor

`description.yml`은 다음을 포함해야 합니다.

- `extension.name`
- `extension.description`
- `extension.version`
- `extension.language`
- `extension.build`
- `extension.license`
- `extension.maintainers`
- `extension.excluded_platforms` 필요 시
- `extension.requires_toolchains` 필요 시
- `repo.github`
- `repo.ref`
- `docs.hello_world`
- `docs.extended_description`

현재 `libdplyr` 기준 권장 값:

- `language: Rust & C++`
- `build: cmake`
- `requires_toolchains: rust`
- `excluded_platforms: windows_amd64_rtools`

`repo.ref`는 **community-extensions CI가 실제로 빌드할 커밋 해시**여야 합니다. 보통은:

1. 저장소 PR CI가 green인 커밋
2. 가능하면 `main`에 머지된 커밋 또는 릴리스 태그가 가리키는 커밋

### 2. 저장소 메타데이터

제출 전에는 최소한 아래가 실제 저장소 URL을 가리켜야 합니다.

- [Cargo.toml](/Users/cypark/Documents/project/libdplyr/Cargo.toml)
- [extension_config.cmake](/Users/cypark/Documents/project/libdplyr/extension_config.cmake)

### 3. 호환성 설명

제출 설명과 PR 본문에서는 이렇게 쓰는 편이 맞습니다.

- `DuckDB 1.5.2`를 현재 community submission target으로 사용
- 저장소 CI에서 `DuckDB 1.4.0`을 별도 compatibility lane으로 계속 검증
- `libdplyr`는 C++/unstable API extension이므로 배포 모델상 최신 stable 대상 제출이 기준

## 실제 제출 순서

1. `libdplyr` 저장소에서 제출 대상 커밋을 확정합니다.
2. `community-pr/description.yml`의 `repo.ref`를 그 커밋으로 고정합니다.
3. `duckdb/community-extensions`를 포크합니다.
4. `extensions/dplyr/description.yml`로 파일 하나만 추가합니다.
5. PR 본문에 다음을 명시합니다.

- `libdplyr`는 Rust core + C API + DuckDB C++ extension 구조
- 현재 최신 stable DuckDB를 대상으로 community build를 요청
- 저장소 CI에서 `1.4.0` compatibility lane을 별도 유지 중
- parser extension 기능은 auto-detection 대상이 아니므로 `extended_description`에 설명을 넣었음

## `ref_next`는 언제 쓰나

DuckDB 다음 minor 릴리스 전환기에는 `ref_next`가 필요할 수 있습니다. 공식 문서 기준으로 release-near 시점엔 latest stable과 DuckDB `main`을 함께 시험하는 흐름이 생깁니다.

지금처럼 `1.5.2` latest stable 기준 제출을 준비하는 단계에선 필수는 아닙니다. 다만 새 DuckDB minor 릴리스가 임박하면:

- `ref`: latest stable 대응 커밋
- `ref_next`: DuckDB `main` 대응 커밋

구조로 가져가는 것이 맞습니다.

## 제출 전 체크

- [ ] PR CI에서 `ubuntu/macos/windows` `DuckDB 1.5.2`가 통과
- [ ] `DuckDB 1.4.0` compatibility lane 통과
- [ ] `community-pr/description.yml` 필드 최신화
- [ ] `repo.ref`를 실제 제출 커밋으로 고정
- [ ] 저장소 URL placeholder 제거
- [ ] parser extension 성격을 `extended_description`에 충분히 설명

## 참고

- [DuckDB Community Extensions 문서](https://duckdb.org/community_extensions/documentation)
- [Community Extension Development](https://duckdb.org/community_extensions/development)
- [DuckDB Release Cycle](https://duckdb.org/docs/lts/dev/release_cycle)
- [Versioning of Extensions](https://duckdb.org/docs/current/extensions/versioning_of_extensions.html)
