// This file is generated from Rust shell DTOs. Do not edit by hand.

export type PeerEndpoint = { v: number, addr: string, peer_id: string, device_id: string, quic_cert_der_b64: string, quic_cert_fingerprint: string, };

export type ProfileSummary = { home: string, peer_id: string, device_id: string, default_room: string, authority_peer_id: string, };

export type MessageView = { event_id: string, created_ms: number, author_peer_id: string, text: string, edited_ms: number | null, redacted: boolean, mentions: Array<string>, thread_root_event_id: string | null, reply_count: number, pinned: boolean, reactions: Array<ReactionView>, attachments: Array<AttachmentView>, };

export type ReactionView = { emoji: string, peer_ids: Array<string>, };

export type AttachmentView = { event_id: string, filename: string, mime: string, sha256: string, data_b64: string, };

export type ChannelView = { room_id: string, name: string, topic: string, visibility: string, selected: boolean, unread_count: number, };

export type RoleView = { role_id: string, name: string, permissions: Array<string>, member_count: number, };

export type ProfileView = { peer_id: string, display_name: string, about: string, };

export type SearchResultView = { room_id: string, message: MessageView, };

export type NotificationView = { event_id: string, room_id: string, author_peer_id: string, summary: string, kind: string, created_ms: number, };

export type CallSignalView = { event_id: string, kind: string, call_id: string, author_peer_id: string, target_peer_id: string | null, video: boolean | null, sdp: string | null, candidate: string | null, created_ms: number, };

export type CallView = { call_id: string, participants: Array<string>, signals: Array<CallSignalView>, };

export type PeerRecord = { v: number, label: string | null, space_id: string, governance_room_id: string, default_room: string, authority_peer_id: string, endpoint: PeerEndpoint, };

export type UiOntologyView = { places: Array<UiPlace>, views: Array<UiView>, commands: Array<UiCommand>, semantic_tokens: Array<SemanticToken>, metrics: Array<UiMetric>, behaviors: Array<UiBehavior>, renderers: Array<UiRenderer>, };

export type ProductGenerationV1 = { v: number, ontology: UiOntologyView, };

export type ProductGenerationStatusView = { kernel_version: string, active_release_id: string, active_sequence: bigint, source: string, previous_available: boolean, update_authentication_available: boolean, available_release_id: string | null, available_sequence: bigint | null, staged_release_id: string | null, staged_sequence: bigint | null, phase: string, notice: string | null, };

export type UiPlace = { id: string, label: string, description: string, editable: boolean, editing_surface: string, };

export type UiView = { id: string, label: string, default_place_id: string, place_id: string, order: number, visible: boolean, description: string, editable: boolean, editing_surface: string, };

export type UiCommand = { id: string, label: string, description: string, scope: UiCommandScope, shortcut: string | null, palette: boolean, editable: boolean, editing_surface: string, };

export type UiCommandScope = "shell" | "frontend";

export type UiViewPlacement = { view_id: string, place_id: string, order: number, visible: boolean, };

export type SemanticToken = { id: string, label: string, default_value: string, current_value: string, used_by: Array<string>, editable: boolean, editing_surface: string, };

export type UiMetric = { id: string, label: string, default_value: number, current_value: number, unit: string, used_by: Array<string>, editable: boolean, editing_surface: string, };

export type UiBehavior = { id: string, label: string, default_value: UiBehaviorValue, current_value: UiBehaviorValue, used_by: Array<string>, editable: boolean, editing_surface: string, };

export type UiRenderer = { id: string, label: string, renders: string, default_renderer: string, current_renderer: string, editable: boolean, editing_surface: string, };

export type UiBehaviorValue = { "type": "bool", "value": boolean } | { "type": "text", "value": string };

export type ShellSnapshotView = { home_root: string, home: HomeScreenView | null, home_error: string | null, network_health: NetworkHealthView, ui_ontology: UiOntologyView, product_generation: ProductGenerationStatusView, service_activity: Array<ServiceActivityItem>, search_results: Array<SearchResultView>, };

export type ServiceActivityItem = { id: number, level: ServiceActivityLevel, summary: string, };

export type ServiceActivityLevel = "info" | "error";

export type InitHomeRequest = { default_room: string | null, };

export type StartServiceRequest = { bind: string | null, advertise: string | null, };

export type SendMessageRequest = { text: string, room: string | null, mentions: Array<string>, thread_root_event_id: string | null, };

export type SelectChannelRequest = { room_id: string, };

export type MarkReadRequest = { room_id: string | null, };

export type CreateChannelRequest = { name: string, topic: string, private_members: Array<string>, };

export type RotateChannelKeyRequest = { room_id: string, };

export type CallJoinRequest = { room: string | null, video: boolean, };

export type CallSignalRequest = { room: string | null, call_id: string, target_peer_id: string, signal_type: string, sdp: string | null, candidate: string | null, };

export type CallLeaveRequest = { room: string | null, call_id: string, };

export type MessageTargetRequest = { target_event_id: string, room: string | null, };

export type EditMessageRequest = { target_event_id: string, text: string, room: string | null, mentions: Array<string>, };

export type ReactionRequest = { target_event_id: string, emoji: string, room: string | null, };

export type AttachmentRequest = { filename: string, mime: string, data_b64: string, room: string | null, };

export type ProfileUpdateRequest = { display_name: string, about: string, };

export type CreateRoleRequest = { name: string, permissions: Array<string>, };

export type AssignRoleRequest = { peer_id: string, role_id: string, };

export type BanMemberRequest = { peer_id: string, reason: string, };

export type SearchMessagesRequest = { query: string, room: string | null, limit: number | null, };

export type ImportPeerRecordRequest = { peer_record_json: string, };

export type CreateSpaceInviteRequest = { expires_minutes: number | null, };

export type JoinSpaceRequest = { space_invite_json: string, max_events: number | null, };

export type PeerCommandRequest = { peer_id: string, device_id: string, max_events: number | null, };

export type SetUiPreferenceRequest = { "kind": "semantic_token", id: string, value: string, } | { "kind": "metric", id: string, value: number, } | { "kind": "behavior", id: string, value: UiBehaviorValue, };

export type SetWorkbenchLayoutRequest = { placements: Array<UiViewPlacement>, };

export type InstallProductUpdateRequest = { package_json: string, };

export type HomeScreenView = { profile: ProfileSummary, runtime: RuntimeStatusView, invite: InviteExchangeView | null, peers: Array<PeerListItemView>, channels: Array<ChannelView>, roles: Array<RoleView>, profiles: Array<ProfileView>, notifications: Array<NotificationView>, call: CallView, room: RoomTimelineView, };

export type NetworkHealthView = { rows: Array<NetworkHealthRow>, };

export type NetworkHealthRow = { id: string, label: string, status: NetworkHealthStatus, summary: string, primary_action: string | null, details: Array<string>, related_views: Array<string>, related_commands: Array<string>, };

export type NetworkHealthStatus = "unknown" | "working" | "needs_attention" | "broken";

export type RuntimeStatusView = { state: RuntimeState, listen_addr: string | null, advertised_addr: string | null, reachability_notes: Array<string>, };

export type RuntimeState = "offline" | "online";

export type InviteExchangeView = { peer_record: PeerRecord, peer_record_json: string, space_invite_json: string | null, };

export type PeerListItemView = { label: string, peer_id: string, device_id: string, addr: string, default_room: string, };

export type RoomTimelineView = { room_id: string, messages: Array<MessageView>, };

export type ShellError = { message: string, };
