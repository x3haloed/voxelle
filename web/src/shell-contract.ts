// This file is generated from Rust shell DTOs. Do not edit by hand.

export type PeerEndpoint = { v: number, addr: string, peer_id: string, device_id: string, quic_cert_der_b64: string, quic_cert_fingerprint: string, };

export type ProfileSummary = { home: string, peer_id: string, device_id: string, default_room: string, authority_peer_id: string, };

export type MessageView = { event_id: string, created_ms: number, author_peer_id: string, text: string, };

export type PeerRecord = { v: number, label: string | null, default_room: string, endpoint: PeerEndpoint, };

export type UiOntologyView = { places: Array<UiPlace>, views: Array<UiView>, commands: Array<UiCommand>, semantic_tokens: Array<SemanticToken>, metrics: Array<UiMetric>, behaviors: Array<UiBehavior>, renderers: Array<UiRenderer>, };

export type UiPlace = { id: string, label: string, description: string, editable: boolean, editing_surface: string, };

export type UiView = { id: string, label: string, place_id: string, description: string, editable: boolean, editing_surface: string, };

export type UiCommand = { id: string, label: string, description: string, editable: boolean, editing_surface: string, };

export type SemanticToken = { id: string, label: string, default_value: string, current_value: string, used_by: Array<string>, editable: boolean, editing_surface: string, };

export type UiMetric = { id: string, label: string, default_value: number, current_value: number, unit: string, used_by: Array<string>, editable: boolean, editing_surface: string, };

export type UiBehavior = { id: string, label: string, default_value: UiBehaviorValue, current_value: UiBehaviorValue, used_by: Array<string>, editable: boolean, editing_surface: string, };

export type UiRenderer = { id: string, label: string, renders: string, default_renderer: string, current_renderer: string, editable: boolean, editing_surface: string, };

export type UiBehaviorValue = { "type": "bool", "value": boolean } | { "type": "text", "value": string };

export type ShellSnapshotView = { home_root: string, home: HomeScreenView | null, home_error: string | null, network_health: NetworkHealthView, ui_ontology: UiOntologyView, service_activity: Array<ServiceActivityItem>, };

export type ServiceActivityItem = { id: number, level: ServiceActivityLevel, summary: string, };

export type ServiceActivityLevel = "info" | "error";

export type InitHomeRequest = { default_room: string | null, };

export type StartServiceRequest = { bind: string | null, advertise: string | null, };

export type SendMessageRequest = { text: string, room: string | null, };

export type ImportPeerRecordRequest = { peer_record_json: string, };

export type PeerCommandRequest = { peer_id: string, device_id: string, max_events: number | null, };

export type HomeScreenView = { profile: ProfileSummary, runtime: RuntimeStatusView, invite: InviteExchangeView | null, peers: Array<PeerListItemView>, room: RoomTimelineView, };

export type NetworkHealthView = { rows: Array<NetworkHealthRow>, };

export type NetworkHealthRow = { id: string, label: string, status: NetworkHealthStatus, summary: string, primary_action: string | null, details: Array<string>, related_views: Array<string>, related_commands: Array<string>, };

export type NetworkHealthStatus = "unknown" | "working" | "needs_attention" | "broken";

export type RuntimeStatusView = { state: RuntimeState, listen_addr: string | null, advertised_addr: string | null, reachability_notes: Array<string>, };

export type RuntimeState = "offline" | "online";

export type InviteExchangeView = { peer_record: PeerRecord, peer_record_json: string, };

export type PeerListItemView = { label: string, peer_id: string, device_id: string, addr: string, default_room: string, diagnostic_state: PeerActionState, sync_state: PeerActionState, };

export type PeerActionState = "not_run";

export type RoomTimelineView = { room_id: string, messages: Array<MessageView>, };

export type ShellError = { message: string, };
