export type SpaceId = string
export type RoomId = string
export type MessageId = string

export type Space = {
  id: SpaceId
  name: string
}

export type RoomVisibility = 'public' | 'private'

export type Room = {
  id: RoomId
  spaceId: SpaceId
  name: string
  visibility: RoomVisibility
}

export type Message = {
  id: MessageId
  spaceId: SpaceId
  roomId: RoomId
  author: string
  text: string
  ts: number
  meta?: {
    eventId?: string
    principalId?: string
    deviceId?: string
  }
}
