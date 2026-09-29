# MoveMgr

여러 프로젝트의 소스 폴더에서 조건에 맞는 파일을 찾아 타겟 폴더로 분류·이동하는 Windows/macOS 데스크톱 앱입니다. Tauri 2, Svelte 5, TypeScript, Rust로 구현되어 로컬에서 작동합니다.

- 앱 이름: **MoveMgr** (작업명)
- 최초 앱 버전: **ver.0.0.1** / 현재 버전: [`version.json`](version.json) 기준
- 채택 구성: **Tauri 2 + Svelte 5 + TypeScript + Rust**, Vite, CSS 디자인 토큰
- 기본 언어: 한국어 / 테마: 시스템 설정에 따른 밝게·어둡게

## 구현된 기능

- 여러 프로젝트 생성·복제·삭제·순서 변경과 체크 상태 영속화
- 게시판형 한 줄 목록에서 프로젝트 이름, 소스 폴더, 소스 조건, 타겟 폴더, 타겟 조건을 한눈에 확인하고 각각 눌러 바로 변경
- 소스 조건과 타겟 조건은 해당 행의 버튼에서 작은 설정 팝업으로 편집
- 소스·타겟 조건을 별도 태그로 저장하고 클릭 한 번으로 적용, 태그 이름 변경·내용 갱신·삭제 지원
- 필수 설정이 비어 있는 프로젝트와 그 프로젝트가 포함된 실행 범위의 실행 버튼 자동 비활성화
- 전체 실행, 체크한 프로젝트 실행, 행별 단일 실행
- 확장자 다중 조건, 파일명 조건, 재귀·숨김 파일 설정
- 이름 전체, 앞 N글자, 구분자 앞까지 분류값 추출
- 타겟 루트, 고정 하위 폴더, 기존 폴더 매칭, 분류값 폴더 생성
- 실행 전 무변경 미리보기와 소스·타겟 중첩 검사
- 동명 파일 건너뛰기 또는 번호 붙이기, 기존 파일 비덮어쓰기
- 동일 볼륨 no-clobber 이동과 볼륨 간 임시 복사·SHA-256 검증·원본 삭제
- 실행 취소, 원본 변경 감지, 원본 삭제 실패 별도 상태, 실행 저널·기록
- UTF-8 JSON 설정 가져오기·내보내기, 교체·추가 가져오기
- 밝은/어두운/시스템 테마, 단일 인스턴스
- 소스 변경 빌드의 patch 자동 증가와 Windows/macOS CI 빌드 구성

## 실행과 빌드

필수 도구는 Node.js, pnpm, Rust stable이며 Windows에서는 Microsoft C++ Build Tools와 WebView2, macOS에서는 Xcode Command Line Tools가 필요합니다.

```powershell
pnpm install
pnpm desktop:dev
pnpm portable:build
```

Windows 포터블 배포본은 항상 `artifacts/portable/MoveMgr-win-x64/`에 생성됩니다. 폴더 안의 `MoveMgr.exe`를 바로 실행하면 되며, 프로젝트 설정·체크 상태·실행 기록은 실행 파일 옆에 자동 생성되는 `MoveMgrData`에 저장됩니다. 새 빌드는 실행 파일을 갱신하고 `MoveMgrData`는 보존합니다. 기존 앱이 실행 중이라 파일이 잠기면 같은 폴더에 `MoveMgr-<version>.exe`를 생성하고 그 경로를 출력합니다. 폴더 전체를 복사하면 설정도 함께 이동합니다. 이전 버전별 폴더의 설정을 옮길 때는 해당 폴더의 `MoveMgrData`를 새 포터블 폴더에 복사하세요.

macOS universal 앱은 macOS에서 `pnpm desktop:build:mac`으로 빌드합니다. 설정은 `.app`과 같은 위치의 `MoveMgrData`에 저장됩니다.

## 검증

```powershell
pnpm check
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
pnpm version:check
```

Windows에서 프런트엔드 검사·빌드, Rust 규칙 및 임시 폴더 이동 테스트, 포터블 release 실행을 검증합니다. macOS용 코드는 같은 저장소와 CI 구성을 사용하지만 macOS 실제 빌드·실행 검증은 macOS runner에서 수행해야 합니다.

## 읽는 순서

1. [English manual](docs/MANUAL.md): 짧은 사용자 설명서
2. [전체 설계](docs/DESIGN.md): 제품 동작, 화면, 조건, 이동 안정성, 빌드·버전 정책
3. [데이터 및 명령 계약](docs/CONTRACTS.md): 설정 모델, 저장, IPC, 상태·오류
4. [구현 모델 인계서](docs/IMPLEMENTATION_HANDOFF.md): 구현 순서와 인수 기준
5. [화면 시안](docs/ui-preview.html): 구현 전 작성한 디자인 목업
6. [설정 예시](docs/examples/settings-export.json): 소스 분류값으로 타겟 하위 폴더를 찾는 예제
7. [아이콘 원본](design/movemgr-icon.svg): 두 폴더 사이의 이동을 표현한 벡터 아이콘

화면 시안은 가상 데이터이며 실제 앱은 `src/`와 `src-tauri/`에 있습니다. 아이콘의 벡터 원본은 `design/movemgr-icon.svg`, 패키지용 생성 파일은 `src-tauri/icons/`에 있습니다.

앱 버전과 설정 스키마 버전은 서로 독립적입니다. 현재 앱 버전은 `version.json`이 기준이고 설정 스키마는 1입니다. 동작 소스가 바뀐 다음 `pnpm desktop:dev` 또는 `pnpm desktop:build`를 실행하면 patch가 한 번 증가하며, 같은 소스의 재빌드는 증가하지 않습니다. major/minor 변경은 `pnpm version:set 0.1.0`처럼 명시적으로 수행합니다.
