export type DelegationCertV1 = {
  v: 1
  principal_id: string
  principal_pub: string
  device_pub: string
  device_id: string
  not_before_ts: number
  expires_ts: number
  scopes: string[]
  sig: string
}

export type EventV1 = {
  v: 1
  space_id: string
  room_id: string
  event_id: string
  author_principal_id: string
  author_device_id: string
  author_device_pub: string
  delegation: DelegationCertV1
  ts: number
  kind: string
  prev: string[]
  body: unknown
  sig: string
}

