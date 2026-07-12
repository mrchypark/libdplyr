# Submodules

이 프로젝트는 빌드/테스트 재현성을 위해 Git submodule을 “커밋 단위로 고정(pinned)”해서 사용합니다.

## 정책

- submodule 업데이트는 **명시적인 PR**로만 수행합니다.
- submodule 커밋이 바뀌면 릴리즈 노트(예: `RELEASE_NOTES_*.md`)에 변경 사실을 적습니다.
- `git submodule update --remote`는 의도치 않은 업데이트를 유발할 수 있으므로 사용을 피합니다.
- Makefile에 포함된 upstream 타겟(`update`, `pull`)은 `--remote`를 사용할 수 있으니, 대신 `make submodules` / `make submodules-init`을 사용합니다.

## 현재 고정 상태

- `duckdb`: `08e34c447bae34eaee3723cac61f2878b6bdf787` (`v1.5.4`)
- `extension-ci-tools`: `72e76e99cd7fee45a99739cd118ec2db64e034ec` (`v1.5-variegata`)

## Compatibility Matrix

- 최소 지원 DuckDB 버전: `v1.4.0`
- 주 지원 DuckDB 버전: `v1.5.4`
- 최소 호환성 검사 버전: `v1.4.0`
- CI는 현재 `v1.5.4`를 멀티플랫폼 주 검증선으로, `v1.4.0`을 Linux 호환성 검증선으로 사용합니다.
- 릴리스 바이너리는 현재 `v1.4.0`과 `v1.5.4`를 대상으로 생성합니다.
- 호환성 매트릭스와 로컬 개발용 submodule 핀은 별개로 관리하되, 현재 DuckDB 핀은 주 지원 버전 `v1.5.4`와 일치합니다.
