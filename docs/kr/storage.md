# S3 호환 Storage Resource (v0)

마나필드의 스토리지 지원은 **Resource/Capability** 구조를 사용한다.
Cloudflare R2는 첫 번째 구현 대상이지만, Module 계약에 R2 제품명을 넣지 않는다.

```text
Module (requires.storage.object)
  └─ Instance bindings.media → manafield-r2 (Resource Instance)
       └─ S3 Resource Provider (버킷 준비 상태 확인/등록)
            └─ Cloudflare R2 / S3 호환 API
```

파일 바이트는 Module ↔ S3 스토리지로 직접 이동한다. Manafield Core는
바이트를 처리하지 않고, Capability 탐색·인스턴스 바인딩을 담당한다.

## 계약과 설정의 구분

Resource가 제공하는 Capability:

```json
{"id":"storage.object","version":"1.0.0"}
```

현 단계의 `storage.object@1.0.0` 소비 방식은 **S3 호환 API**를 전제로
한다. 단순히 파일 저장 기능이 있다는 이유로 S3 비호환 저장소에
같은 계약을 광고하면 안 된다. 나중에 독립적인 스토리지 API나
SDK 추상화가 생기면 별도 계약 버전으로 정리한다.

Module의 `manafield.module.json`:

```json
{
  "requires": {
    "capabilities": {
      "media": {"id": "storage.object", "version": "^1.0.0"}
    }
  }
}
```

Instance Definition(비밀정보 제외):

```yaml
resources:
  - id: manafield-r2
    provider: s3
    config:
      endpoint: https://example-account.r2.cloudflarestorage.com
      bucket: example-media
      region: auto
      forcePathStyle: true
modules:
  - id: trusted-media-module
    # source와 build 생략
    bindings:
      media: manafield-r2
```

`media`는 Module의 로컬 Requirement 슬롯이고 `manafield-r2`는
구체적인 Resource Instance ID다. 하나의 S3 Resource를 여러
**신뢰된** Module이 바인딩할 수 있다.

## 비밀정보 및 접근 권한

S3 API Access Key ID와 Secret Access Key는 Instance 루트의
`secrets/s3/<resource-id>/` 아래에 비공개 파일로 보관한다.
Resource 설명, Build Plan, GitHub, 일반 환경 변수에는 값을 넣지 않는다.

배포 CLI는 Module에 S3 엔드포인트·버킷·리전·오브젝트 키 prefix와
키 파일 경로를 전달한다. 비밀번호 파일 자체는 읽기 전용 바인드
마운트로 전달한다.

**prefix는 접근 권한의 격리 수단이 아니다.** 같은 버킷 전체에
접근할 수 있는 키를 여러 Module에 지급하면 서로의 prefix에도
접근할 수 있다. v0는 신뢰한 Module 전용이며, 향후 제3자 Module
지원 시에는 Module별 제한 자격증명 또는 prefix 검증 게이트웨이가
필요하다.

R2의 S3 API 엔드포인트와 공개 미디어 URL은 다르다.
공개 업로드를 위해 무조건 `public-read` S3 ACL을 전송하지 않는다.
공개 URL이 필요하다면 별도로 CDN/커스텀 도메인을 설정한다.

## 현재 지원 범위

- S3 Resource Provider: 버킷 목록 접근 확인, Core 등록 및 재등록
- 직접 배포 CLI: PostgreSQL 최대 1개 + S3 최대 1개 Resource
- Resource 설정 검증: HTTPS 엔드포인트, 버킷, 리전, 비밀 설정 차단
- `storage.object` 요구사항 미바인딩·오바인딩 시 직접 배포 거부
- 기존 Jenkins v0: S3 배포를 **명시적으로 거부** (별도 지원 예정)
- 실제 R2 연결, S3 객체의 읽기·쓰기 테스트, 녹템블리스 본체의
  S3 설정 소비는 별도 통합 검증 대상

자세한 실행 방식과 테스트는
[providers/s3/README.md](../../providers/s3/README.md)를 참고한다.
