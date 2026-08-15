const {
  shell,
  app,
} = api;

const ROLE_PERMISSIONS = [
  "message:post",
  "message:moderate",
  "message:pin",
  "channel:manage",
  "role:manage",
  "member:ban",
  "invite:create",
];

if (!(app instanceof HTMLElement)) {
  throw new Error("missing #app");
}

const uiState = {
  busyCommand: "",
  error: "",
  errorRecovery: "",
  errorDetail: "",
  status: "",
  peerRecordDraft: "",
  spaceInviteDraft: "",
  messageDraft: "",
  messageMentionsDraft: new Set(),
  replyTargetEventId: "",
  replyPreview: null,
  editingMessageId: "",
  messageEditDraft: "",
  messageEditMentionsDraft: new Set(),
  deletingMessageId: "",
  banningPeerId: "",
  channelNameDraft: "",
  channelTopicDraft: "",
  channelPrivateDraft: false,
  channelMembersDraft: new Set(),
  roleNameDraft: "",
  rolePermissionsDraft: new Set(),
  roleCreateOpen: false,
  roleAssignmentDraft: null,
  profileNameDraft: "",
  profileAboutDraft: "",
  profileDraftInitialized: false,
  searchDraft: "",
  bindDraft: "",
  advertiseDraft: "",
  productUpdateDraft: "",
  trustTransitionDraft: "",
  draggedViewId: "",
  layoutEditing: false,
  connectionOpen: false,
  utilityOpen: "",
  paletteOpen: false,
  paletteQuery: "",
  localMediaStream: null,
  localMediaMode: "",
  remoteMediaStreams: new Map(),
  peerConnections: new Map(),
  peerConnectionStates: new Map(),
  pendingIce: new Map(),
  seenCallSignals: new Set(),
  processingCallSignals: false,
  mediaNotice: null,
  lastCallHeartbeatMs: 0,
  preparingHomeRecovery: false,
  homeRecoveryNotice: "",
};
const focusCoordinator = new FocusSurfaceCoordinator(
  document,
  (callback) => window.requestAnimationFrame(callback),
);

const viewRenderers = {
  "profile.summary": profileSummaryView,
  "identity.recovery": identityRecoveryView,
  "runtime.status": runtimeStatusView,
  "network.health": networkHealthView,
  "field.test": fieldTestView,
  "product.update": productUpdateView,
  "invite.exchange": inviteExchangeView,
  "peer.list": peerListView,
  "channel.list": channelListView,
  "member.profiles": memberProfilesView,
  "role.list": roleListView,
  "message.search": messageSearchView,
  "notification.center": notificationCenterView,
  "room.timeline": roomTimelineView,
  "message.composer": messageComposerView,
  "call.mesh": callMeshView,
  "service.activity": activityView,
};

let currentSnapshot = await shell.execute("shell.refresh");
let refreshInFlight = false;
let refreshQueued = false;
if (
  ontologyPresentation(currentSnapshot.ui_ontology).startOnlineOnLaunch
  && currentSnapshot.home?.runtime.state === "offline"
) {
  currentSnapshot = await shell.execute("runtime.goOnline", {
    bind: null,
    advertise: null,
  });
}
render();

const stopSnapshotInvalidation = await shell.onSnapshotInvalidated(() => {
  publishRefresh().catch(reportError);
});

async function publishRefresh() {
  if (refreshInFlight || uiState.busyCommand) {
    refreshQueued = true;
    return;
  }
  refreshInFlight = true;
  try {
    do {
      refreshQueued = false;
      await refresh();
    } while (refreshQueued);
    render();
  } finally {
    refreshInFlight = false;
  }
}

const heartbeatTimer = window.setInterval(async () => {
  const localPeerId = currentSnapshot.home?.profile.peer_id;
  const call = currentSnapshot.home?.call;
  if (
    refreshInFlight
    || uiState.busyCommand
    || !uiState.localMediaStream
    || !localPeerId
    || !call?.participants.includes(localPeerId)
    || Date.now() - uiState.lastCallHeartbeatMs < 20_000
  ) return;
  try {
    currentSnapshot = await shell.execute("call.heartbeat", {
      room: currentSnapshot.home?.room.room_id ?? null,
      call_id: call.call_id,
    });
    uiState.lastCallHeartbeatMs = Date.now();
    render();
  } catch (error) {
    reportError(error);
  }
}, 1_000);

async function refresh() {
  currentSnapshot = await shell.execute("shell.refresh");
  return currentSnapshot;
}

function render() {
  assertSnapshotContract(currentSnapshot);
  const localPeerId = currentSnapshot.home?.profile.peer_id;
  if (
    uiState.localMediaStream
    && localPeerId
    && !currentSnapshot.home?.call?.participants.includes(localPeerId)
  ) {
    stopLocalMedia();
    uiState.mediaNotice = "Call session ended or the four-peer mesh was full.";
  }
  for (const peerId of disconnectedParticipantIds(
    currentSnapshot.home?.call?.participants ?? [],
    uiState.peerConnections.keys(),
  )) {
    closePeerConnection(peerId);
  }
  const presentation = applyOntology(
    document.documentElement,
    currentSnapshot.ui_ontology,
  );
  const desired = document.createElement("div");
  desired.append(
    header(currentSnapshot),
    globalErrorBanner(),
    globalStatusBanner(),
    ...(currentSnapshot.home && !currentSnapshot.home.recovery.kit_exported
      ? [recoverySetupPrompt()]
      : []),
    ...(uiState.connectionOpen && currentSnapshot.home
      ? [connectionCenter(currentSnapshot)]
      : []),
    ...(uiState.utilityOpen && currentSnapshot.home
      ? [utilityCenter(currentSnapshot, uiState.utilityOpen)]
      : []),
    currentSnapshot.home
      ? workbenchShell(currentSnapshot)
      : onboardingExperience(currentSnapshot),
    ...(uiState.paletteOpen ? [commandPalette(currentSnapshot)] : []),
  );
  reconcileChildren(app, desired);
  synchronizeTransientFocus();
  if (presentation.activityAutoScroll) {
    const activity = app.querySelector(".activity-list");
    activity?.scrollTo?.({ top: activity.scrollHeight });
  }
  attachCallMedia();
  processCallSignals().catch(reportError);
}

function assertSnapshotContract(snapshot) {
  if (snapshot.home && !snapshot.home.recovery) {
    throw new Error(
      "Voxelle shell snapshot is missing home.recovery; rebuild the native kernel and product component together.",
    );
  }
}

function recoverySetupPrompt() {
  const prompt = element("section", "recovery-setup-prompt");
  prompt.setAttribute("aria-label", "Recovery setup required");
  const copy = element("div", "");
  copy.append(
    element("strong", "", "Protect this identity before relying on this device"),
    element(
      "p",
      "summary",
      "Save an offline recovery kit. It is the only supported path back to the same principal after local loss.",
    ),
  );
  prompt.append(copy, commandButton("identity.recovery.export"));
  return prompt;
}

function handleKeydown(event) {
  if (event.key === "Tab" && uiState.paletteOpen) {
    const palette = app.querySelector(".command-palette");
    if (palette) trapModalTab(event, palette);
    return;
  }
  if (event.key === "Escape" && uiState.paletteOpen) {
    event.preventDefault();
    uiState.paletteOpen = false;
    render();
    return;
  }
  if (event.key === "Escape" && (uiState.connectionOpen || uiState.utilityOpen)) {
    event.preventDefault();
    uiState.connectionOpen = false;
    uiState.utilityOpen = "";
    render();
    return;
  }
  const command = currentSnapshot.ui_ontology.commands.find((candidate) =>
    shortcutMatches(event, candidate.shortcut)
  );
  if (!command) return;
  event.preventDefault();
  runCommand(command.id).catch(reportError);
}
document.addEventListener("keydown", handleKeydown);

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function header(snapshot) {
  const headerEl = element("header", "app-header");
  const titleGroup = element("div", "title-group");
  titleGroup.append(element("h1", "", "Voxelle"));
  const selectedChannel = snapshot.home?.channels.find((channel) => channel.selected);
  titleGroup.append(element(
    "p",
    "header-context",
    selectedChannel
      ? `${selectedChannel.visibility === "private" ? "Private · " : "# "}${selectedChannel.name}`
      : "Private communication, owned by its members",
  ));

  const actions = element("div", "header-actions");
  if (snapshot.home) {
    actions.append(
      connectionCenterButton(snapshot),
      utilityButton("people", `People · ${snapshot.home.profiles.length}`),
      utilityButton(
        "notifications",
        snapshot.home.notifications.length > 0
          ? `Notifications · ${snapshot.home.notifications.length}`
          : "Notifications",
      ),
      utilityButton("search", "Search"),
    );
  }
  if ((shell.mode ?? "unknown") !== "tauri") actions.append(shellMode());
  actions.append(headerMore(snapshot));

  headerEl.append(titleGroup, actions);
  return headerEl;
}

function connectionCenterButton(snapshot) {
  const status = connectionHeaderState(snapshot);
  const button = actionButton(status.label, () => {
    if (!uiState.connectionOpen) rememberFocusReturn();
    uiState.utilityOpen = "";
    uiState.connectionOpen = !uiState.connectionOpen;
    render();
  }, status.help);
  button.classList.add("connection-button");
  button.dataset.status = status.tone;
  button.setAttribute("aria-expanded", String(uiState.connectionOpen));
  button.setAttribute("aria-controls", "connection-center");
  return button;
}

function headerMore(snapshot) {
  const details = element("details", "header-more");
  details.append(element("summary", "command-button", "More"));
  const menu = element("div", "header-more-menu");
  if (snapshot.home) {
    menu.append(
      actionButton("Customize", () => {
        rememberFocusReturn();
        uiState.connectionOpen = false;
        uiState.utilityOpen = "settings";
        render();
      }),
      layoutEditorButton(),
    );
  }
  menu.append(
    commandButton("workbench.commandPalette.open"),
    commandButton("shell.refresh"),
  );
  details.append(menu);
  return details;
}

function utilityButton(kind, label) {
  const button = actionButton(label, () => {
    if (uiState.utilityOpen !== kind) rememberFocusReturn();
    uiState.connectionOpen = false;
    uiState.utilityOpen = uiState.utilityOpen === kind ? "" : kind;
    render();
  });
  button.setAttribute("aria-expanded", String(uiState.utilityOpen === kind));
  button.setAttribute("aria-controls", "utility-center");
  return button;
}

function utilityCenter(snapshot, kind) {
  const definitions = {
    people: {
      title: "People",
      summary: "Your profile, members, and invitations for this space.",
      render: () => {
        const content = element("div", "utility-sections");
        content.append(
          utilitySection("You", profileSummaryView(snapshot)),
          utilitySection("Members", memberProfilesView(snapshot)),
          utilitySection("Roles and access", roleListView(snapshot)),
          utilitySection("Invite people", inviteExchangeView(snapshot)),
        );
        return content;
      },
    },
    notifications: {
      title: "Notifications",
      summary: "Unread mentions retained from your replicated channels.",
      render: () => notificationCenterView(snapshot),
    },
    search: {
      title: "Search messages",
      summary: "Search retained messages and attachment names on this device.",
      render: () => messageSearchView(snapshot),
    },
    settings: {
      title: "Customize Voxelle",
      summary: "Choose everyday behavior first. Advanced appearance and spacing remain available when you want them.",
      render: () => customizationEditor(snapshot),
    },
  };
  const definition = definitions[kind] ?? definitions.people;
  const aside = element("aside", "connection-center utility-center");
  aside.id = "utility-center";
  aside.setAttribute("role", "dialog");
  aside.setAttribute("aria-modal", "false");
  aside.setAttribute("aria-labelledby", "utility-center-title");
  const heading = element("div", "connection-center-heading");
  const copy = element("div", "panel-title");
  const title = element("h2", "", definition.title);
  title.id = "utility-center-title";
  copy.append(title, element("p", "summary", definition.summary));
  const close = actionButton("Close", () => {
    uiState.utilityOpen = "";
    render();
  });
  close.dataset.dialogInitialFocus = "true";
  heading.append(copy, close);
  const body = element("div", "connection-center-body");
  body.append(definition.render());
  aside.append(heading, body);
  return aside;
}

function utilitySection(title, content) {
  const section = element("section", "utility-section");
  section.append(element("h3", "", title), content);
  return section;
}

function connectionCenter(snapshot) {
  const aside = element("aside", "connection-center");
  aside.id = "connection-center";
  aside.setAttribute("role", "dialog");
  aside.setAttribute("aria-modal", "false");
  aside.setAttribute("aria-labelledby", "connection-center-title");
  const heading = element("div", "connection-center-heading");
  const copy = element("div", "panel-title");
  const title = element("h2", "", "Connection & sync");
  title.id = "connection-center-title";
  copy.append(
    title,
    element(
      "p",
      "summary",
      "Voxelle tries ordinary peers automatically. Change addresses here when intervention is needed; setup checks stay distinct from broken states.",
    ),
  );
  const close = actionButton("Close", () => {
    uiState.connectionOpen = false;
    render();
  });
  close.dataset.dialogInitialFocus = "true";
  heading.append(copy, close);
  const body = element("div", "connection-center-body");
  body.append(serviceOptions(), networkHealthView(snapshot));
  aside.append(heading, body);
  return aside;
}

function onboardingExperience(snapshot) {
  if (snapshot.home_error) return damagedHomeExperience(snapshot.home_error);
  const section = element("section", "onboarding");
  section.setAttribute("aria-labelledby", "onboarding-title");
  const intro = element("div", "onboarding-intro");
  intro.append(
    element("p", "eyebrow", "Private communication, owned by its members"),
    element("h2", "", "How would you like to begin?"),
    element(
      "p",
      "summary",
      "Voxelle creates identity and space authority on your devices. No Voxelle service owns your account, membership, messages, or recovery.",
    ),
  );
  intro.querySelector("h2").id = "onboarding-title";

  const choices = element("div", "onboarding-choices");
  if (uiState.homeRecoveryNotice) {
    intro.append(element("p", "recovery-success", uiState.homeRecoveryNotice));
  }
  const create = onboardingChoice(
    "Create a new space",
    "Start a new identity and private space on this device. You can invite people after Voxelle brings your peer online.",
  );
  create.append(commandButton("home.init"));

  const join = onboardingChoice(
    "Join with an invite",
    "A signed invite grants membership and includes several ordinary peers Voxelle can try automatically—even when the inviter is offline.",
  );
  const joinForm = element("form", "field-stack");
  joinForm.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("space.join").catch(reportError);
  });
  const inviteFile = document.createElement("input");
  inviteFile.type = "file";
  inviteFile.accept = ".voxinvite,application/json";
  inviteFile.setAttribute("aria-label", "Choose a signed Voxelle invite");
  inviteFile.addEventListener("change", async () => {
    const file = inviteFile.files?.[0];
    if (!file) return;
    uiState.spaceInviteDraft = await file.text();
    inviteText.value = uiState.spaceInviteDraft;
    updateInviteReview(inviteReview, uiState.spaceInviteDraft);
  });
  const inviteText = element("textarea", "invite-input");
  inviteText.rows = 5;
  inviteText.placeholder = "Paste signed .voxinvite JSON";
  inviteText.setAttribute("aria-label", "Signed Voxelle invite");
  inviteText.value = uiState.spaceInviteDraft;
  inviteText.addEventListener("input", () => {
    uiState.spaceInviteDraft = inviteText.value;
    updateInviteReview(inviteReview, uiState.spaceInviteDraft);
  });
  const inviteReview = element("div", "invite-review");
  updateInviteReview(inviteReview, uiState.spaceInviteDraft);
  joinForm.append(inviteFile, inviteText, inviteReview, submitButton("space.join"));
  join.append(joinForm);

  const recover = onboardingChoice(
    "Recover my identity",
    "Use an offline recovery kit after losing a device or local state. Recovery preserves your principal, rotates authority to this device, and resynchronizes retained history.",
  );
  recover.append(
    element(
      "p",
      "recovery-note",
      "Your .voxrecover file is a bearer capability. Voxelle reads it locally and never uploads it.",
    ),
    commandButton("identity.recovery.restore"),
  );

  choices.append(create, join, recover);
  section.append(intro, choices);
  return section;
}

function updateInviteReview(container, text) {
  container.replaceChildren(inviteReviewContent(inviteClaimPreview(text)));
}

function inviteReviewContent(preview) {
  if (preview.state === "empty") {
    return element("p", "muted", "Choose or paste an invite to review its claims before joining.");
  }
  if (preview.state === "unavailable") {
    const message = element("p", "invite-review-warning", preview.reason);
    message.setAttribute("role", "status");
    return message;
  }
  const review = element("section", "invite-review-claims");
  review.setAttribute("aria-label", "Untrusted invite claims");
  const expires = preview.expiresMs === null
    ? "Missing or unrecognized"
    : new Date(preview.expiresMs).toLocaleString();
  review.append(
    element("strong", "", "Review before joining"),
    definitionGrid([
      ["Space", preview.spaceName],
      ["Space ID", preview.spaceId],
      ["Authority", preview.authorityPeerId],
      ["Expires", expires],
      ["Included peers", preview.bootstrapCount === null ? "Unrecognized" : String(preview.bootstrapCount)],
    ]),
    element(
      "p",
      "recovery-note",
      "This is a bearer invite. Anyone holding an unbound copy may attempt to join more than once until it expires or members revoke it.",
    ),
    element(
      "p",
      "summary",
      "These are untrusted claims for review. Rust verifies the space genesis, authority signature, expiry, governance history, and peer records when you choose Join Space.",
    ),
  );
  if (preview.expiredClaim || !preview.claimsConsistent) {
    const warning = element(
      "p",
      "invite-review-warning",
      preview.expiredClaim
        ? "This invite claims it has expired. Rust will make the authoritative decision."
        : "The displayed authority and space claims conflict. Rust is expected to reject this invite.",
    );
    warning.setAttribute("role", "alert");
    review.prepend(warning);
  }
  return review;
}

function damagedHomeExperience(homeError) {
  const section = element("section", "onboarding damaged-home");
  section.setAttribute("aria-labelledby", "damaged-home-title");
  const panel = element("article", "damaged-home-panel");
  panel.append(
    element("p", "eyebrow", "Local recovery needed"),
    element("h2", "", "This local home cannot be opened"),
    element("p", "summary", homeError.message),
    element("p", "recovery-note", homeError.recovery_message),
  );
  panel.querySelector("h2").id = "damaged-home-title";
  const details = element("details", "technical-details");
  details.append(
    element("summary", "", "Technical details"),
    element("pre", "", homeError.detail),
  );
  panel.append(details);

  if (uiState.preparingHomeRecovery) {
    const confirmation = element("section", "home-recovery-confirmation");
    confirmation.setAttribute("role", "alertdialog");
    confirmation.setAttribute("aria-label", "Prepare this device for identity recovery confirmation");
    confirmation.append(
      element("strong", "", "Prepare this device for recovery?"),
      element("p", "summary", "Voxelle will move the unusable local identity, device certificate, and retained database into a private archive inside this home. Nothing is deleted."),
      element("p", "recovery-note", "You need an offline .voxrecover kit to preserve the same identity. Without one, stop here and retain this archive for diagnosis."),
    );
    const controls = element("div", "row-actions");
    const confirm = commandButton("home.archiveForRecovery");
    confirm.textContent = "Archive local state and continue";
    confirm.dataset.dialogInitialFocus = "true";
    controls.append(confirm, actionButton("Cancel", () => {
      uiState.preparingHomeRecovery = false;
      render();
      window.requestAnimationFrame(() => app.querySelector(".damaged-home-panel > .command-button")?.focus());
    }));
    confirmation.append(controls);
    panel.append(confirmation);
  } else {
    const prepare = actionButton("Prepare This Device for Recovery…", () => {
      uiState.preparingHomeRecovery = true;
      render();
      window.requestAnimationFrame(() => app.querySelector(".home-recovery-confirmation .command-button")?.focus());
    });
    panel.append(prepare);
  }
  section.append(panel);
  return section;
}

function onboardingChoice(title, description) {
  const article = element("article", "onboarding-choice");
  article.append(
    element("h3", "", title),
    element("p", "summary", description),
  );
  return article;
}

function layoutEditorButton() {
  const button = actionButton(
    uiState.layoutEditing ? "Finish layout" : "Edit layout",
    () => {
      uiState.layoutEditing = !uiState.layoutEditing;
      render();
    },
  );
  button.setAttribute("aria-pressed", String(uiState.layoutEditing));
  return button;
}

function shellMode() {
  const mode = shell.mode ?? "unknown";
  return element(
    "div",
    `shell-mode ${mode}`,
    mode === "tauri" ? "Tauri" : "Preview only · no peer service",
  );
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function customizationEditor(snapshot) {
  const fragment = document.createDocumentFragment();
  fragment.append(
    preferenceGroup("Everyday behavior", snapshot.ui_ontology.behaviors, behaviorEditor),
  );
  const advanced = element("details", "settings-advanced");
  advanced.append(element("summary", "command-button", "Advanced appearance and spacing"));
  const editor = element("div", "customization-editor");
  editor.append(
    preferenceGroup("Appearance", snapshot.ui_ontology.semantic_tokens, semanticTokenEditor),
    preferenceGroup("Spacing and size", snapshot.ui_ontology.metrics, metricEditor),
  );
  advanced.append(editor);
  fragment.append(advanced, commandButton("ui.preferences.reset"));
  return fragment;
}

function preferenceGroup(title, preferences, renderer) {
  const group = element("section", "preference-group");
  group.append(element("h3", "", title));
  for (const preference of preferences.filter((item) => item.editable)) {
    group.append(renderer(preference));
  }
  return group;
}

/** @param {import("./shell-contract").SemanticToken} token */
function semanticTokenEditor(token) {
  const input = preferenceInput(token, "text", token.current_value);
  return preferenceForm(token, input, () => ({
    kind: "semantic_token",
    id: token.id,
    value: input.value,
  }), true);
}

/** @param {import("./shell-contract").UiMetric} metric */
function metricEditor(metric) {
  const input = preferenceInput(metric, "number", String(metric.current_value));
  input.min = "0";
  input.step = metric.unit === "count" ? "1" : "0.5";
  return preferenceForm(metric, input, () => ({
    kind: "metric",
    id: metric.id,
    value: input.valueAsNumber,
  }), true);
}

/** @param {import("./shell-contract").UiBehavior} behavior */
function behaviorEditor(behavior) {
  const value = behavior.current_value;
  const input = behavior.id === "timestamps.style"
    ? timestampStyleInput(value.type === "text" ? value.value : "relative")
    : preferenceInput(
      behavior,
      value.type === "bool" ? "checkbox" : "text",
      value.type === "text" ? value.value : "",
    );
  if (value.type === "bool") {
    input.checked = value.value;
  }
  return preferenceForm(behavior, input, () => ({
    kind: "behavior",
    id: behavior.id,
    value: value.type === "bool"
      ? { type: "bool", value: input.checked }
      : { type: "text", value: input.value },
  }), false);
}

function timestampStyleInput(value) {
  const select = element("select", "preference-input");
  for (const [optionValue, label] of [["relative", "Relative time"], ["absolute", "Date and time"]]) {
    const option = element("option", "", label);
    option.value = optionValue;
    option.selected = optionValue === value;
    select.append(option);
  }
  return select;
}

function preferenceInput(preference, type, value) {
  const input = element("input", "preference-input");
  input.type = type;
  input.value = value;
  input.dataset.preferenceId = preference.id;
  return input;
}

function preferenceForm(preference, input, request, showId) {
  const form = element("form", "preference-form");
  form.dataset.preferenceId = preference.id;
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("ui.preference.set", request()).catch(reportError);
  });
  const label = element("label", "preference-label");
  label.append(element("span", "", preference.label));
  if (showId) label.append(element("small", "view-id", preference.id));
  label.append(input);
  const save = submitButton("ui.preference.set");
  save.setAttribute("aria-label", `Save ${preference.label}`);
  form.append(label, save);
  return form;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function workbenchShell(snapshot) {
  const container = element("section", "workbench-container");
  const hidden = snapshot.ui_ontology.views.filter((view) => !view.visible);
  if (uiState.layoutEditing && hidden.length > 0) {
    const shelf = element("nav", "hidden-view-shelf");
    shelf.dataset.renderKey = "hidden-view-shelf";
    shelf.setAttribute("aria-label", "Hidden workbench views");
    shelf.append(element("span", "muted", "Hidden views"));
    for (const view of hidden) {
      shelf.append(actionButton(`Show ${view.label}`, () => {
        saveLayout(setViewVisible(currentSnapshot.ui_ontology.views, view.id, true)).catch(reportError);
      }));
    }
    shelf.append(commandButton("workbench.layout.reset"));
    container.append(shelf);
  }

  const shellEl = element("section", "workbench");
  shellEl.dataset.renderKey = "workbench";
  const occupiedPlaces = new Set(snapshot.ui_ontology.views
    .filter((view) => view.visible)
    .map((view) => view.place_id));
  if (!occupiedPlaces.has("inspector")) shellEl.classList.add("without-inspector");
  for (const place of snapshot.ui_ontology.places) {
    if (!uiState.layoutEditing && !occupiedPlaces.has(place.id)) continue;
    shellEl.append(dockZone(place, snapshot));
  }
  container.append(shellEl);
  return container;
}

function dockZone(place, snapshot) {
  const zone = element("section", `dock-zone dock-${place.id}`);
  zone.dataset.renderKey = `dock:${place.id}`;
  zone.dataset.placeId = place.id;
  zone.setAttribute("aria-label", `${place.label} dock`);
  if (uiState.layoutEditing) {
    const heading = element("div", "dock-zone-header");
    heading.append(
      element("span", "dock-zone-label", place.label),
      element("span", "view-id", place.id),
    );
    zone.append(heading);
    zone.addEventListener("dragover", (event) => {
      event.preventDefault();
      zone.dataset.dragOver = "true";
    });
    zone.addEventListener("dragleave", () => delete zone.dataset.dragOver);
    zone.addEventListener("drop", (event) => {
      event.preventDefault();
      delete zone.dataset.dragOver;
      const viewId = event.dataTransfer?.getData("text/x-voxelle-view")
        || uiState.draggedViewId;
      if (viewId) {
        saveLayout(moveView(currentSnapshot.ui_ontology.views, viewId, place.id)).catch(reportError);
      }
    });
  }

  const views = snapshot.ui_ontology.views
    .filter((view) => view.visible && view.place_id === place.id)
    .sort((left, right) => left.order - right.order || left.id.localeCompare(right.id));
  if (views.length === 0) {
    if (uiState.layoutEditing) {
      zone.append(element("p", "dock-empty", "Drop a view here"));
    }
  } else {
    for (const view of views) {
      zone.append(workbenchPanel(view, snapshot));
    }
  }
  return zone;
}

/**
 * @param {import("./shell-contract").UiView} viewDefinition
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 */
function workbenchPanel(viewDefinition, snapshot) {
  const section = element("section", "panel");
  section.dataset.renderKey = `view:${viewDefinition.id}`;
  section.dataset.panelId = `panel.${viewDefinition.id}`;
  section.dataset.viewId = viewDefinition.id;
  section.dataset.placeId = viewDefinition.place_id;
  section.append(panelHeader(viewDefinition, snapshot));

  const view = element("div", "panel-view");
  const renderer = viewRenderers[viewDefinition.id] ?? unknownView;
  view.append(renderer(snapshot));
  section.append(view);
  return section;
}

function panelHeader(viewDefinition, snapshot) {
  const headerEl = element("div", "panel-header");
  const titleGroup = element("div", "panel-title");
  titleGroup.append(element("h2", "", viewDefinition.label));
  if (!uiState.layoutEditing) {
    headerEl.append(titleGroup);
    return headerEl;
  }

  headerEl.draggable = true;
  headerEl.addEventListener("dragstart", (event) => {
    uiState.draggedViewId = viewDefinition.id;
    event.dataTransfer?.setData("text/x-voxelle-view", viewDefinition.id);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  });
  headerEl.addEventListener("dragend", () => {
    uiState.draggedViewId = "";
  });
  titleGroup.append(element("span", "view-id", viewDefinition.id));
  const controls = element("div", "panel-controls");
  const placeSelect = element("select", "dock-select");
  placeSelect.setAttribute("aria-label", `Dock ${viewDefinition.label}`);
  for (const place of snapshot.ui_ontology.places) {
    const option = element("option", "", place.label);
    option.value = place.id;
    option.selected = place.id === viewDefinition.place_id;
    placeSelect.append(option);
  }
  placeSelect.addEventListener("pointerdown", (event) => event.stopPropagation());
  placeSelect.addEventListener("change", () => {
    saveLayout(moveView(currentSnapshot.ui_ontology.views, viewDefinition.id, placeSelect.value))
      .catch(reportError);
  });
  controls.append(
    actionButton("↑", () => {
      saveLayout(shiftView(currentSnapshot.ui_ontology.views, viewDefinition.id, -1)).catch(reportError);
    }, `Move ${viewDefinition.label} earlier`),
    actionButton("↓", () => {
      saveLayout(shiftView(currentSnapshot.ui_ontology.views, viewDefinition.id, 1)).catch(reportError);
    }, `Move ${viewDefinition.label} later`),
    placeSelect,
    actionButton("×", () => {
      saveLayout(setViewVisible(currentSnapshot.ui_ontology.views, viewDefinition.id, false))
        .catch(reportError);
    }, `Hide ${viewDefinition.label}`),
  );
  headerEl.append(titleGroup, controls);
  return headerEl;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function profileSummaryView(snapshot) {
  const fragment = document.createDocumentFragment();
  if (!snapshot.home) {
    const empty = element("div", "empty-state");
    empty.append(
      element("h3", "", "No initialized home"),
      element("p", "summary", snapshot.home_error?.message ?? "Home state is not available."),
      commandButton("home.init"),
    );
    const joinForm = element("form", "field-stack");
    joinForm.addEventListener("submit", (event) => {
      event.preventDefault();
      runCommand("space.join").catch(reportError);
    });
    const inviteInput = element("textarea", "peer-record-input");
    inviteInput.placeholder = "Paste signed .voxinvite JSON";
    inviteInput.value = uiState.spaceInviteDraft;
    inviteInput.addEventListener("input", () => {
      uiState.spaceInviteDraft = inviteInput.value;
    });
    joinForm.append(
      element("h3", "", "Or join a space"),
      element("p", "summary", "Paste the signed invite; Voxelle will create your identity, join, sync, and go online."),
      inviteInput,
      submitButton("space.join"),
    );
    empty.append(joinForm);
    fragment.append(empty);
    return fragment;
  }

  const profile = snapshot.home.profile;
  const projected = snapshot.home.profiles.find((candidate) =>
    candidate.peer_id === profile.peer_id
  ) ?? {
    peer_id: profile.peer_id,
    display_name: `Peer ${shortId(profile.peer_id)}`,
    about: "",
  };
  if (!uiState.profileDraftInitialized) {
    uiState.profileNameDraft = projected.display_name;
    uiState.profileAboutDraft = projected.about;
    uiState.profileDraftInitialized = true;
  }
  const identity = element("div", "identity-card");
  identity.append(
    element("div", "profile-avatar", profileInitials(projected.display_name)),
    element("div", "identity-copy"),
  );
  identity.lastElementChild.append(
    element("strong", "profile-name", projected.display_name),
    element(
      "p",
      "summary",
      projected.about || "Add a name and a short note so members recognize you.",
    ),
    element(
      "p",
      "identity-stats",
      `${snapshot.home.channels.length} channel${snapshot.home.channels.length === 1 ? "" : "s"} · ${snapshot.home.peers.length} connection${snapshot.home.peers.length === 1 ? "" : "s"}`,
    ),
  );

  const edit = element("details", "advanced-details profile-edit");
  edit.append(element("summary", "", "Edit your profile"));
  const form = element("form", "field-stack");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("profile.update").catch(reportError);
  });
  form.append(
    labeledInput(
      "Display name",
      "Your name",
      uiState.profileNameDraft,
      (value) => { uiState.profileNameDraft = value; },
    ),
    labeledInput(
      "About",
      "A short profile",
      uiState.profileAboutDraft,
      (value) => { uiState.profileAboutDraft = value; },
    ),
    submitButton("profile.update"),
  );
  edit.append(form);

  const advanced = element("details", "advanced-details");
  advanced.append(
    element("summary", "", "Identity details"),
    definitionGrid([
      ["Principal", profile.peer_id],
      ["This device", profile.device_id],
      ["Authority", profile.authority_peer_id],
      ["Home", snapshot.home_root],
      ["Default room", profile.default_room],
    ]),
  );

  fragment.append(identity, edit, advanced);

  return fragment;
}

function identityRecoveryView(snapshot) {
  const fragment = document.createDocumentFragment();
  if (snapshot.home) {
    const warning = element("div", "recovery-warning");
    warning.append(
      element(
        "h3",
        "",
        snapshot.home.recovery.kit_exported
          ? "Recovery kit saved"
          : "Keep this capability offline",
      ),
      element(
        "p",
        "summary",
        snapshot.home.recovery.kit_exported
          ? "Voxelle recorded that a recovery kit was saved. Keep it protected and offline; save a fresh copy whenever your recovery plan changes."
          : "Anyone holding this file can rotate your identity authority away from every current device. Save it to protected offline storage; do not send it as a message or keep it beside this computer.",
      ),
    );
    fragment.append(warning, commandButton("identity.recovery.export"));
  } else {
    const copy = element("div", "empty-state");
    copy.append(
      element("h3", "", "Recover the same identity"),
      element(
        "p",
        "summary",
        "Choose your offline .voxrecover file. Voxelle will rotate authority to this device, revoke the lost devices, recover private-channel keys, and resynchronize from ordinary retaining peers when they are available.",
      ),
      element(
        "p",
        "summary",
        "Recovery only works in a fresh Voxelle home and never sends the recovery file to a service.",
      ),
      commandButton("identity.recovery.restore"),
    );
    fragment.append(copy);
  }
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function runtimeStatusView(snapshot) {
  const runtime = snapshot.home?.runtime;
  const rows = [
    ["Runtime", runtime?.state ?? "offline"],
    ["Listen", runtime?.listen_addr ?? "not listening"],
    ["Advertise", runtime?.advertised_addr ?? "not advertising"],
  ];
  for (const [index, note] of (runtime?.reachability_notes ?? []).entries()) {
    rows.push([`Reachability ${index + 1}`, note]);
  }
  const fragment = document.createDocumentFragment();
  const controls = element("div", "control-row");
  controls.append(
    commandButton("home.init"),
    commandButton("runtime.goOnline"),
    commandButton("runtime.goOffline"),
  );
  fragment.append(definitionGrid(rows), serviceOptions(), controls);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function networkHealthView(snapshot) {
  const fragment = document.createDocumentFragment();
  const controls = element("div", "control-row");
  controls.append(
    commandButton("home.init"),
    commandButton("runtime.goOnline"),
    commandButton("runtime.goOffline"),
  );
  fragment.append(controls);

  const rows = element("ol", "health-list");
  for (const row of snapshot.network_health.rows) {
    rows.append(healthRow(row));
  }
  fragment.append(rows);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function productUpdateView(snapshot) {
  const generation = snapshot.product_generation;
  const fragment = document.createDocumentFragment();
  fragment.append(definitionGrid([
    ["Kernel", generation.kernel_version],
    ["Generation", generation.active_release_id],
    ["Sequence", String(generation.active_sequence)],
    ["Source", generation.source],
    ["Update state", generation.phase],
    ["Available", generation.available_release_id
      ? `${generation.available_release_id} · sequence ${generation.available_sequence}`
      : "none discovered"],
    ["Staged", generation.staged_release_id
      ? `${generation.staged_release_id} · sequence ${generation.staged_sequence}`
      : "none"],
    ["Signed updates", generation.update_authentication_available ? "available" : "no trusted release root embedded"],
    ["Trusted release keys", String(generation.trusted_update_key_count)],
    ["Trust sequence", String(generation.trust_sequence)],
  ]));
  if (generation.notice) {
    fragment.append(element("p", "notice", generation.notice));
  }
  fragment.append(commandButton("product.update.check"));
  if (generation.available_release_id && generation.phase === "available") {
    fragment.append(commandButton("product.update.stageAvailable"));
  }
  if (generation.staged_release_id) {
    fragment.append(commandButton("product.update.activateStaged"));
    fragment.append(commandButton("product.update.discardStaged"));
  }

  const form = element("form", "field-stack");
  const packageInput = element("textarea", "mono-input");
  packageInput.rows = 7;
  packageInput.placeholder = "Paste a signed .voxupdate package";
  packageInput.value = uiState.productUpdateDraft;
  packageInput.setAttribute("aria-label", "Signed product update package");
  packageInput.addEventListener("input", () => {
    uiState.productUpdateDraft = packageInput.value;
  });
  const fileInput = document.createElement("input");
  fileInput.type = "file";
  fileInput.accept = ".voxupdate,application/json";
  fileInput.setAttribute("aria-label", "Choose a signed product update package");
  fileInput.addEventListener("change", async () => {
    const file = fileInput.files?.[0];
    if (!file) return;
    uiState.productUpdateDraft = await file.text();
    packageInput.value = uiState.productUpdateDraft;
  });
  form.append(
    fileInput,
    packageInput,
    element("p", "summary", "The native kernel verifies the embedded release signature before staging or activation. GitHub and mirrors are transport only."),
    submitButton("product.update.install"),
  );
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("product.update.install").catch(reportError);
  });
  fragment.append(form);
  const trustForm = element("form", "field-stack");
  const trustInput = element("textarea", "mono-input");
  trustInput.rows = 4;
  trustInput.placeholder = "Paste a signed .voxtrust transition";
  trustInput.value = uiState.trustTransitionDraft;
  trustInput.setAttribute("aria-label", "Signed release trust transition");
  trustInput.addEventListener("input", () => {
    uiState.trustTransitionDraft = trustInput.value;
  });
  trustForm.append(
    trustInput,
    element("p", "summary", "Trust transitions are ordered, signed by a currently trusted release key, and can add or retire release keys without trusting GitHub."),
    submitButton("product.update.rotateTrust"),
  );
  trustForm.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("product.update.rotateTrust").catch(reportError);
  });
  fragment.append(trustForm);
  if (generation.previous_available) {
    fragment.append(commandButton("product.update.rollback"));
  }
  return fragment;
}

function serviceOptions() {
  const options = element("div", "service-options");
  options.append(
    labeledInput("Bind", "Optional local bind, e.g. [::]:0", uiState.bindDraft, (value) => {
      uiState.bindDraft = value;
    }),
    labeledInput(
      "Advertise",
      "Optional advertised IPv6 address",
      uiState.advertiseDraft,
      (value) => {
        uiState.advertiseDraft = value;
      },
    ),
  );
  return options;
}

/**
 * @param {import("./shell-contract").NetworkHealthRow} row
 */
function healthRow(row) {
  const item = element("li", "health-row");
  item.dataset.renderKey = `health:${row.id}`;
  item.dataset.status = row.status;
  item.dataset.healthRowId = row.id;

  const indicator = element("span", "status-indicator", statusLabel(row.status));
  const body = element("div", "health-body");
  body.append(element("h3", "", row.label));
  body.append(element("p", "summary", row.summary));

  if (row.details.length > 0) {
    const details = element("ul", "details");
    for (const detail of row.details) {
      details.append(element("li", "", detail));
    }
    body.append(details);
  }

  item.append(indicator, body);
  if (row.primary_action) {
    item.append(commandButton(row.primary_action, row.primary_action_payload ?? undefined));
  }
  return item;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function activityView(snapshot) {
  const fragment = document.createDocumentFragment();
  const actions = element("div", "control-row");
  actions.append(commandButton("shell.refresh"));
  fragment.append(actions);

  const list = element("ol", "activity-list");
  for (const activity of visibleActivity(
    snapshot.service_activity,
    snapshot.ui_ontology,
  )) {
    const row = element("li", "");
    row.dataset.renderKey = `activity:${activity.id}`;
    row.dataset.level = activity.level;
    row.append(
      element("span", "activity-id", String(activity.id)),
      element("span", "", activity.summary),
    );
    list.append(row);
  }
  fragment.append(list);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function inviteExchangeView(snapshot) {
  const fragment = document.createDocumentFragment();
  const invite = snapshot.home?.invite?.space_invite_json ?? "";
  const inviteGroup = element("div", "invite-flow");
  if (invite) {
    inviteGroup.append(
      element("p", "success-label", "Invite ready"),
      element(
        "p",
        "summary",
        "Send this signed membership invite to one person. It is valid for 24 hours and includes ordinary peers Voxelle can try automatically.",
      ),
      commandButton("invite.copy"),
      commandButton("space.invite.create"),
    );
    const details = element("details", "advanced-details");
    details.append(
      element("summary", "", "Signed invite details"),
      element("pre", "invite-json", invite),
    );
    inviteGroup.append(details);
  } else {
    inviteGroup.append(
      element("h3", "", "Invite someone to this space"),
      element(
        "p",
        "summary",
        snapshot.home?.runtime.state === "online"
          ? "Create a signed invite, then copy it into a private message. Voxelle includes multiple known peers when available so joining does not depend on you staying online."
          : "Go online first so the invite can include reachable ordinary peers.",
      ),
      snapshot.home?.runtime.state === "online"
        ? commandButton("space.invite.create")
        : commandButton("runtime.goOnline"),
    );
  }

  const manual = element("details", "advanced-details");
  manual.append(element("summary", "", "Manual peer setup"));
  const importGroup = element("form", "field-stack");
  importGroup.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("peer.import").catch(reportError);
  });
  const textarea = element("textarea", "peer-record-input");
  textarea.placeholder = "Paste peer record JSON";
  textarea.value = uiState.peerRecordDraft;
  textarea.addEventListener("input", () => {
    uiState.peerRecordDraft = textarea.value;
  });
  importGroup.append(
    element(
      "p",
      "summary",
      "Peer records provide availability only; they never grant membership. Use this when diagnostics require a peer address outside an invite.",
    ),
    textarea,
    submitButton("peer.import"),
  );
  manual.append(importGroup);

  fragment.append(inviteGroup, manual);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function fieldTestView(snapshot) {
  const fragment = document.createDocumentFragment();
  const rows = [
    {
      label: "Home initialized",
      status: snapshot.home ? "working" : "needs_attention",
      command: snapshot.home ? null : "home.init",
      detail: snapshot.home ? snapshot.home.profile.default_room : snapshot.home_error?.message,
    },
    {
      label: "Resident service online",
      status: snapshot.home?.runtime.state === "online" ? "working" : "needs_attention",
      command: snapshot.home?.runtime.state === "online" ? "runtime.goOffline" : "runtime.goOnline",
      detail: snapshot.home?.runtime.advertised_addr ?? "offline",
    },
    {
      label: "Invite available",
      status: snapshot.home?.invite?.space_invite_json ? "working" : "unknown",
      command: snapshot.home?.invite?.space_invite_json
        ? "invite.copy"
        : snapshot.home?.invite
          ? "space.invite.create"
          : "runtime.goOnline",
      detail: snapshot.home?.invite?.space_invite_json
        ? "signed membership capability ready"
        : "go online, then create a signed invite",
    },
    {
      label: "Peer imported",
      status: (snapshot.home?.peers.length ?? 0) > 0 ? "working" : "needs_attention",
      command: "peer.import",
      detail: `${snapshot.home?.peers.length ?? 0} known peer(s)`,
    },
    {
      label: "Peer diagnostic",
      status: activityIncludes(snapshot, "diagnostic reached") ? "working" : "needs_attention",
      command: (snapshot.home?.peers.length ?? 0) > 0 ? "peer.diagnose" : "peer.import",
      detail: activityIncludes(snapshot, "diagnostic reached")
        ? "latest diagnostic reached a peer"
        : "run against an imported peer",
    },
    {
      label: "Room sync",
      status: activityIncludes(snapshot, "sync") ? "working" : "needs_attention",
      command: (snapshot.home?.peers.length ?? 0) > 0 ? "peer.sync" : "peer.import",
      detail: `${snapshot.home?.room.messages.length ?? 0} visible message(s)`,
    },
  ];

  const list = element("ol", "workflow-list");
  for (const row of rows) {
    const item = element("li", "workflow-row");
    item.dataset.renderKey = `workflow:${row.label}`;
    item.dataset.status = row.status;
    const body = element("div", "workflow-body");
    body.append(
      element("span", "status-indicator", statusLabel(row.status)),
      element("h3", "", row.label),
      element("p", "summary", row.detail ?? ""),
    );
    item.append(body);
    if (row.command) {
      item.append(commandButton(row.command));
    }
    list.append(item);
  }

  fragment.append(list);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function peerListView(snapshot) {
  const peers = snapshot.home?.peers ?? [];
  const list = element("ol", "peer-list");
  for (const peer of peers) {
    const row = element("li", "peer-row");
    row.dataset.renderKey = `peer:${peer.peer_id}:${peer.device_id}`;
    const body = element("div", "peer-body");
    body.append(
      element("strong", "", peer.label),
      element("span", "muted", "Available for direct connection and retained-history sync"),
    );
    const details = element("details", "advanced-details peer-details");
    const actions = element("div", "row-actions");
    actions.append(
      commandButton("peer.diagnose", peerRequest(peer)),
      commandButton("peer.sync", peerRequest(peer)),
    );
    details.append(
      element("summary", "", "Connection details"),
      definitionGrid([
        ["Address", peer.addr],
        ["Principal", peer.peer_id],
        ["Device", peer.device_id],
      ]),
      actions,
    );
    row.append(body, details);
    list.append(row);
  }
  return list;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function channelListView(snapshot) {
  const fragment = document.createDocumentFragment();
  const list = element("ol", "peer-list");
  for (const channel of snapshot.home?.channels ?? []) {
    const row = element("li", channel.selected ? "peer-row selected" : "peer-row");
    row.dataset.renderKey = `channel:${channel.room_id}`;
    const body = element("div", "peer-body");
    body.append(
      element("strong", "", `${channel.visibility === "private" ? "🔒" : "#"} ${channel.name}${channel.unread_count > 0 ? ` (${channel.unread_count})` : ""}`),
      element("span", "muted", channel.topic),
    );
    const actions = element("div", "row-actions");
    if (!channel.selected) {
      actions.append(commandButton("channel.select", { room_id: channel.room_id }));
    }
    if (channel.visibility === "private") {
      actions.append(commandButton("channel.rotateKey", { room_id: channel.room_id }));
    }
    row.append(body);
    if (actions.children.length > 0) row.append(actions);
    list.append(row);
  }
  const create = element("details", "channel-create advanced-details");
  create.append(element("summary", "", "Create a channel"));
  const form = element("form", "field-stack");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("channel.create").catch(reportError);
  });
  form.append(
    labeledInput("Name", "new-channel", uiState.channelNameDraft, (value) => { uiState.channelNameDraft = value; }),
    labeledInput("Topic", "What belongs here?", uiState.channelTopicDraft, (value) => { uiState.channelTopicDraft = value; }),
  );
  const privacy = element("details", "advanced-details");
  privacy.open = uiState.channelPrivateDraft;
  const privacySummary = element("summary", "", "Private channel options");
  const privateToggle = choiceCheckbox(
    "Only selected members can read this channel",
    uiState.channelPrivateDraft,
    (checked) => {
      uiState.channelPrivateDraft = checked;
      render();
    },
  );
  const memberChoices = element("fieldset", "choice-list");
  memberChoices.append(element("legend", "", "Who can read it?"));
  const eligibleProfiles = (snapshot.home?.profiles ?? []).filter(
    (profile) => !profile.banned && profile.peer_id !== snapshot.home?.profile.peer_id,
  );
  for (const profile of eligibleProfiles) {
    memberChoices.append(choiceCheckbox(
      profile.display_name,
      uiState.channelMembersDraft.has(profile.peer_id),
      (checked) => updateSet(uiState.channelMembersDraft, profile.peer_id, checked),
    ));
  }
  privacy.append(
    privacySummary,
    privateToggle,
    element("p", "summary", "You are always included. Private channel content is encrypted for the selected members."),
  );
  if (uiState.channelPrivateDraft) {
    privacy.append(
      eligibleProfiles.length > 0
        ? memberChoices
        : element("p", "summary", "No other current members. This channel will be private to you."),
    );
  }
  form.append(privacy, submitButton("channel.create"));
  create.append(form);
  fragment.append(list, create);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function memberProfilesView(snapshot) {
  const fragment = document.createDocumentFragment();
  const list = element("ol", "peer-list");
  for (const profile of snapshot.home?.profiles ?? []) {
    const row = element("li", "peer-row");
    row.dataset.renderKey = `profile:${profile.peer_id}`;
    row.dataset.profilePeerId = profile.peer_id;
    row.tabIndex = -1;
    const body = element("div", "member-card");
    const isOwn = profile.peer_id === snapshot.home?.profile.peer_id;
    const copy = element("div", "member-copy");
    copy.append(
      element("strong", "", `${profile.display_name}${isOwn ? " · you" : ""}`),
      element("span", "muted", profile.about),
    );
    const roleNames = profile.role_ids
      .map((roleId) => snapshot.home?.roles.find((role) => role.role_id === roleId)?.name)
      .filter(Boolean);
    copy.append(element(
      "span",
      profile.banned ? "status-badge danger" : "muted",
      profile.banned ? "Banned from this space" : roleNames.join(", ") || "Member",
    ));
    body.append(
      element("div", "profile-avatar small", profileInitials(profile.display_name)),
      copy,
    );
    row.append(body);
    if (!isOwn) {
      const actions = element("details", "advanced-details member-actions");
      actions.append(
        element("summary", "", "Member actions"),
        element(
          "p",
          "summary",
          profile.banned
            ? "This removes the ban. They still need a valid invite to become a member again."
            : "Banning removes this principal's authority to participate; retained history remains authoritative.",
        ),
      );
      if (profile.banned) {
        const allow = commandButton("member.unban", { peer_id: profile.peer_id, reason: "" });
        allow.textContent = `Allow ${profile.display_name} to rejoin`;
        actions.append(allow);
      } else if (uiState.banningPeerId === profile.peer_id) {
        actions.open = true;
        actions.append(memberBanConfirmation(profile));
      } else {
        actions.append(actionButton(`Ban ${profile.display_name} from this space…`, () => {
          beginMemberBan(profile.peer_id);
        }));
      }
      row.append(actions);
    }
    list.append(row);
  }
  fragment.append(list);
  return fragment;
}

function memberBanConfirmation(profile) {
  const confirmation = element("section", "member-ban-confirmation");
  confirmation.setAttribute("role", "alertdialog");
  confirmation.setAttribute("aria-label", `Ban ${profile.display_name} confirmation`);
  confirmation.append(
    element("strong", "", `Ban ${profile.display_name}?`),
    element(
      "p",
      "summary",
      "They will lose authority to participate in this space. Their retained history remains, and allowing them later will still require a new valid invite.",
    ),
  );
  const controls = element("div", "row-actions");
  const confirm = commandButton("member.ban", {
    peer_id: profile.peer_id,
    reason: "Removed by a space administrator",
  });
  confirm.textContent = `Ban ${profile.display_name}`;
  controls.append(confirm, actionButton("Cancel ban", () => cancelMemberBan(profile.peer_id)));
  confirmation.append(controls);
  return confirmation;
}

function beginMemberBan(peerId) {
  uiState.banningPeerId = peerId;
  render();
  window.requestAnimationFrame(() => app.querySelector(".member-ban-confirmation .command-button")?.focus());
}

function cancelMemberBan(peerId) {
  uiState.banningPeerId = "";
  render();
  focusProfileRow(peerId);
}

function focusProfileRow(peerId) {
  window.requestAnimationFrame(() => {
    const row = [...app.querySelectorAll("[data-profile-peer-id]")]
      .find((candidate) => candidate.dataset.profilePeerId === peerId);
    row?.focus();
  });
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function roleListView(snapshot) {
  const fragment = document.createDocumentFragment();
  const list = element("ol", "peer-list");
  for (const role of snapshot.home?.roles ?? []) {
    const row = element("li", "peer-row");
    row.dataset.renderKey = `role:${role.role_id}`;
    row.dataset.roleId = role.role_id;
    row.tabIndex = -1;
    const body = element("div", "peer-body");
    body.append(
      element("strong", "", role.name),
      element("span", "muted", `${role.member_count} ${role.member_count === 1 ? "member" : "members"}`),
      element("span", "muted", role.permissions.map(permissionLabel).join(", ") || "No additional permissions"),
    );
    row.append(body);
    if (role.role_id !== "role:everyone") {
      const members = element("details", "advanced-details");
      members.open = uiState.roleAssignmentDraft?.roleId === role.role_id;
      members.append(element("summary", "", "Manage members"));
      const memberList = element("div", "choice-list");
      for (const profile of snapshot.home?.profiles ?? []) {
        if (profile.banned) continue;
        const assigned = profile.role_ids.includes(role.role_id);
        const draft = uiState.roleAssignmentDraft;
        if (draft?.roleId === role.role_id && draft.peerId === profile.peer_id) {
          memberList.append(roleAssignmentConfirmation(role, profile, draft.grant));
        } else {
          memberList.append(actionButton(
            assigned
              ? `Remove ${profile.display_name} from ${role.name}…`
              : `Give ${role.name} to ${profile.display_name}…`,
            () => beginRoleAssignment(role.role_id, profile.peer_id, !assigned),
          ));
        }
      }
      members.append(memberList);
      row.append(members);
    }
    list.append(row);
  }
  const create = element("details", "advanced-details");
  create.open = uiState.roleCreateOpen;
  create.addEventListener("toggle", () => {
    uiState.roleCreateOpen = create.open;
  });
  create.append(element("summary", "", "Create a role"));
  const form = element("form", "field-stack");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    if (!uiState.roleNameDraft.trim()) {
      setUserError("Enter a role name.");
      render();
      return;
    }
    if (uiState.rolePermissionsDraft.size === 0) {
      setUserError("Choose at least one permission for this role.");
      render();
      return;
    }
    runCommand("role.create", {
      name: uiState.roleNameDraft,
      permissions: [...uiState.rolePermissionsDraft],
    }).catch(reportError);
  });
  form.append(labeledInput("Role name", "For example, Moderator", uiState.roleNameDraft, (value) => {
    uiState.roleNameDraft = value;
  }));
  const permissions = element("fieldset", "choice-list");
  permissions.append(element("legend", "", "What can this role do?"));
  for (const permission of ROLE_PERMISSIONS) {
    permissions.append(choiceCheckbox(
      permissionLabel(permission),
      uiState.rolePermissionsDraft.has(permission),
      (checked) => updateSet(uiState.rolePermissionsDraft, permission, checked),
    ));
  }
  form.append(permissions, submitButton("role.create"));
  create.append(form);
  fragment.append(list, create);
  return fragment;
}

function roleAssignmentConfirmation(role, profile, grant) {
  const verb = grant ? "Give" : "Remove";
  const confirmation = element("section", "role-assignment-confirmation");
  confirmation.setAttribute("role", "alertdialog");
  confirmation.setAttribute(
    "aria-label",
    `${grant ? "Give" : "Remove"} ${role.name} ${grant ? "to" : "from"} ${profile.display_name} confirmation`,
  );
  const permissions = role.permissions.map(permissionLabel).join(", ") || "no additional permissions";
  confirmation.append(
    element(
      "strong",
      "",
      `${verb} ${role.name} ${grant ? "to" : "from"} ${profile.display_name}?`,
    ),
    element(
      "p",
      "summary",
      grant
        ? `${profile.display_name} will gain this role's authority: ${permissions}. Their other roles are unchanged.`
        : `${profile.display_name} will lose this role's authority: ${permissions}. Authority from their other roles is unchanged.`,
    ),
  );
  const controls = element("div", "row-actions");
  const confirm = commandButton(grant ? "role.grant" : "role.revoke", {
    peer_id: profile.peer_id,
    role_id: role.role_id,
  });
  confirm.textContent = grant
    ? `Give ${role.name} to ${profile.display_name}`
    : `Remove ${role.name} from ${profile.display_name}`;
  controls.append(confirm, actionButton("Cancel role change", () => cancelRoleAssignment(role.role_id)));
  confirmation.append(controls);
  return confirmation;
}

function beginRoleAssignment(roleId, peerId, grant) {
  uiState.roleAssignmentDraft = { roleId, peerId, grant };
  render();
  window.requestAnimationFrame(() => app.querySelector(".role-assignment-confirmation .command-button")?.focus());
}

function cancelRoleAssignment(roleId) {
  uiState.roleAssignmentDraft = null;
  render();
  focusRoleRow(roleId);
}

function focusRoleRow(roleId) {
  window.requestAnimationFrame(() => {
    const row = [...app.querySelectorAll("[data-role-id]")]
      .find((candidate) => candidate.dataset.roleId === roleId);
    row?.focus();
  });
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function messageSearchView(snapshot) {
  const fragment = document.createDocumentFragment();
  const form = element("form", "field-stack");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("message.search").catch(reportError);
  });
  form.append(labeledInput("Search", "Words in messages or attachment names", uiState.searchDraft, (value) => { uiState.searchDraft = value; }), submitButton("message.search"));
  const results = element("ol", "message-list");
  for (const result of snapshot.search_results ?? []) {
    const row = element("li", "message remote");
    row.dataset.renderKey = `search:${result.message.event_id}`;
    const author = profileForPeer(snapshot, result.message.author_peer_id);
    const channel = channelName(snapshot, result.room_id);
    row.append(
      element("strong", "", author.display_name),
      element("span", "muted", channel),
      element("p", "", result.message.text),
      actionButton(`Open result in ${channel}`, () => {
        openRetainedMessage(result.room_id, result.message.event_id).catch(reportError);
      }),
    );
    results.append(row);
  }
  fragment.append(form, results);
  return fragment;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function notificationCenterView(snapshot) {
  const fragment = document.createDocumentFragment();
  const list = element("ol", "activity-list");
  const notifications = snapshot.home?.notifications ?? [];
  if (notifications.length === 0) {
    list.append(element("li", "empty-state", "No unread mentions."));
  }
  for (const notification of notifications) {
    const row = element("li", "");
    row.dataset.renderKey = `notification:${notification.event_id}`;
    const author = profileForPeer(snapshot, notification.author_peer_id).display_name;
    const channel = channelName(snapshot, notification.room_id);
    row.append(
      element("strong", "", author),
      element("span", "muted", channel),
      element("span", "", notification.summary),
      actionButton(`Open message from ${author} in ${channel}`, () => {
        openNotification(notification).catch(reportError);
      }),
    );
    list.append(row);
  }
  fragment.append(list);
  return fragment;
}

async function openNotification(notification) {
  await openRetainedMessage(notification.room_id, notification.event_id);
}

async function openRetainedMessage(roomId, eventId) {
  uiState.utilityOpen = "";
  await runCommand("message.open", { room_id: roomId, event_id: eventId });
  window.requestAnimationFrame(() => {
    const message = [...app.querySelectorAll("[data-message-event-id]")]
      .find((candidate) => candidate.dataset.messageEventId === eventId);
    message?.scrollIntoView?.({ block: "center" });
    message?.focus();
  });
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function roomTimelineView(snapshot) {
  const fragment = document.createDocumentFragment();
  const channel = snapshot.home?.channels.find((candidate) => candidate.selected);
  const context = element("div", "conversation-context");
  context.append(
    element("h3", "", channel
      ? `${channel.visibility === "private" ? "🔒 " : "# "}${channel.name}`
      : "Conversation"),
    element("p", "summary", channel?.topic || "Messages retained and synchronized by space members."),
  );
  const messages = snapshot.home?.room.messages ?? [];
  const list = element("ol", "message-list");
  if (messages.length === 0) {
    const empty = element("li", "conversation-empty");
    empty.append(
      element("strong", "", `Start ${channel ? `#${channel.name}` : "the conversation"}`),
      element("p", "summary", "The first accepted message will appear here and synchronize through ordinary retaining peers."),
    );
    list.append(empty);
  }
  for (const message of messages) {
    const own = message.author_peer_id === snapshot.home?.profile.peer_id;
    const row = element("li", own ? "message own" : "message remote");
    row.dataset.renderKey = `message:${message.event_id}`;
    row.dataset.messageEventId = message.event_id;
    row.tabIndex = -1;
    const author = profileForPeer(snapshot, message.author_peer_id);
    const avatar = element("div", "profile-avatar small", profileInitials(author.display_name));
    const content = element("div", "message-content");
    const meta = element("div", "message-meta");
    meta.append(element("strong", "", own ? `${author.display_name} · you` : author.display_name));
    const timestamp = messageTimestamp(message, snapshot.ui_ontology);
    if (timestamp !== null) {
      const time = element("time", "message-time", timestamp);
      time.dateTime = safeDateTime(message.created_ms) ?? "";
      meta.append(time);
    }
    if (uiState.editingMessageId === message.event_id && !message.redacted) {
      content.append(meta, messageEditForm(message, snapshot));
    } else {
      content.append(meta, element("p", message.redacted ? "muted" : "message-text", message.text));
    }
    const annotations = element("div", "message-annotations");
    if (message.edited_ms !== null) annotations.append(element("small", "muted", "edited"));
    if (message.pinned) annotations.append(element("small", "muted", "pinned"));
    if (message.thread_root_event_id !== null) annotations.append(element("small", "muted", "thread reply"));
    if (message.reply_count > 0) annotations.append(element("small", "muted", `${message.reply_count} repl${message.reply_count === 1 ? "y" : "ies"}`));
    if (annotations.children.length > 0) content.append(annotations);
    const reactions = element("div", "message-reactions");
    for (const reaction of message.reactions ?? []) {
      const ownReaction = reaction.peer_ids.includes(snapshot.home?.profile.peer_id ?? "");
      const button = commandButton(ownReaction ? "reaction.remove" : "reaction.add", { target_event_id: message.event_id, emoji: reaction.emoji, room: snapshot.home?.room.room_id ?? null });
      button.textContent = `${reaction.emoji} ${reaction.peer_ids.length}`;
      button.setAttribute("aria-label", `${ownReaction ? "Remove" : "Add"} ${reaction.emoji} reaction`);
      reactions.append(button);
    }
    if (reactions.children.length > 0) content.append(reactions);
    for (const attachment of message.attachments ?? []) {
      const link = element("a", "attachment-link", attachment.filename);
      link.href = `data:${attachment.mime};base64,${attachment.data_b64}`;
      link.download = attachment.filename;
      content.append(link);
    }
    const actionDetails = element("details", "message-actions");
    actionDetails.append(element("summary", "", "Message actions"));
    const actions = element("div", "row-actions");
    const ownThumb = message.reactions
      .find((reaction) => reaction.emoji === "👍")
      ?.peer_ids.includes(snapshot.home?.profile.peer_id ?? "") ?? false;
    const thumb = commandButton(ownThumb ? "reaction.remove" : "reaction.add", {
      target_event_id: message.event_id,
      emoji: "👍",
      room: snapshot.home?.room.room_id ?? null,
    });
    thumb.textContent = ownThumb ? "Remove 👍" : "React 👍";
    const pin = commandButton(message.pinned ? "pin.remove" : "pin.add", {
      target_event_id: message.event_id,
      room: snapshot.home?.room.room_id ?? null,
    });
    actions.append(
      actionButton("Reply", () => beginReply(message, author.display_name)),
      thumb,
      pin,
    );
    if (own && !message.redacted) {
      actions.append(
        actionButton("Edit", () => beginMessageEdit(message)),
        actionButton("Delete…", () => beginMessageDelete(message.event_id)),
      );
    }
    actionDetails.append(actions);
    content.append(actionDetails);
    if (uiState.deletingMessageId === message.event_id) {
      content.append(messageDeleteConfirmation(message, snapshot));
    }
    row.append(avatar, content);
    list.append(row);
  }
  fragment.append(context, list);
  return fragment;
}

function messageDeleteConfirmation(message, snapshot) {
  const confirmation = element("section", "message-delete-confirmation");
  confirmation.setAttribute("role", "alertdialog");
  confirmation.setAttribute("aria-label", "Delete message confirmation");
  confirmation.append(
    element("strong", "", "Delete this message?"),
    element("p", "summary", "Its text will be replaced by a signed tombstone. The deletion remains part of retained history."),
  );
  const controls = element("div", "row-actions");
  const remove = commandButton("message.redact", {
    target_event_id: message.event_id,
    room: snapshot.home?.room.room_id ?? null,
  });
  remove.textContent = "Delete message";
  controls.append(remove, actionButton("Cancel deletion", cancelMessageDelete));
  confirmation.append(controls);
  return confirmation;
}

function beginMessageDelete(eventId) {
  uiState.deletingMessageId = eventId;
  render();
  window.requestAnimationFrame(() => app.querySelector(".message-delete-confirmation .command-button")?.focus());
}

function cancelMessageDelete() {
  uiState.deletingMessageId = "";
  render();
}

function messageEditForm(message, snapshot) {
  const form = element("form", "message-edit-form");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    if (!uiState.messageEditDraft.trim()) return;
    runCommand("message.edit", {
      target_event_id: message.event_id,
      text: uiState.messageEditDraft,
      room: snapshot.home?.room.room_id ?? null,
    }).catch(reportError);
  });
  const input = element("textarea", "message-edit-input");
  input.dataset.syncFocusedValue = "true";
  input.rows = 2;
  input.value = uiState.messageEditDraft;
  input.setAttribute("aria-label", "Edit message");
  input.addEventListener("input", () => {
    uiState.messageEditDraft = input.value;
  });
  input.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      cancelMessageEdit();
    } else if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      if (input.value.trim()) form.requestSubmit();
    }
  });
  const controls = element("div", "row-actions");
  const save = submitButton("message.edit");
  save.textContent = "Save changes";
  controls.append(save, actionButton("Cancel edit", cancelMessageEdit));
  form.append(
    input,
    mentionPicker(input, snapshot, (value, peerId) => {
      uiState.messageEditDraft = value;
      uiState.messageEditMentionsDraft.add(peerId);
    }),
    element("span", "composer-hint", "Enter to save · Escape to cancel"),
    controls,
  );
  return form;
}

function beginMessageEdit(message) {
  uiState.editingMessageId = message.event_id;
  uiState.messageEditDraft = message.text;
  uiState.messageEditMentionsDraft = new Set(message.mentions ?? []);
  render();
  window.requestAnimationFrame(() => app.querySelector(".message-edit-input")?.focus());
}

function cancelMessageEdit() {
  uiState.editingMessageId = "";
  uiState.messageEditDraft = "";
  uiState.messageEditMentionsDraft.clear();
  render();
}

function beginReply(message, authorName) {
  uiState.replyTargetEventId = message.thread_root_event_id ?? message.event_id;
  uiState.replyPreview = {
    authorName,
    text: message.text,
  };
  render();
  window.requestAnimationFrame(() => app.querySelector(".message-input")?.focus());
}

function cancelReply() {
  uiState.replyTargetEventId = "";
  uiState.replyPreview = null;
  render();
  window.requestAnimationFrame(() => app.querySelector(".message-input")?.focus());
}

function messageComposerView(snapshot) {
  const form = element("form", "message-form");
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    runCommand("message.send").catch(reportError);
  });
  const channel = snapshot.home?.channels.find((candidate) => candidate.selected);
  const input = element("textarea", "message-input");
  input.dataset.syncFocusedValue = "true";
  input.rows = 2;
  input.placeholder = `Message ${channel ? `#${channel.name}` : "this room"}`;
  input.setAttribute("aria-label", input.placeholder);
  input.value = uiState.messageDraft;
  const count = element("span", "composer-count", `${uiState.messageDraft.length}`);
  input.addEventListener("input", () => {
    uiState.messageDraft = input.value;
    count.textContent = String(input.value.length);
  });
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      if (input.value.trim()) form.requestSubmit();
    }
  });
  const fileInput = element("input", "");
  fileInput.type = "file";
  fileInput.className = "visually-hidden";
  fileInput.setAttribute("aria-label", "Attach file");
  fileInput.addEventListener("change", async () => {
    const file = fileInput.files?.[0];
    if (!file) return;
    if (file.size > 256 * 1024) {
      reportError(new Error("attachments are limited to 256 KiB"));
      return;
    }
    const data_b64 = await fileAsBase64(file);
    await runCommand("attachment.add", {
      filename: file.name,
      mime: file.type || "application/octet-stream",
      data_b64,
      room: null,
    });
  });
  const controls = element("div", "composer-controls");
  controls.append(
    mentionPicker(input, snapshot, (value, peerId) => {
      uiState.messageDraft = value;
      uiState.messageMentionsDraft.add(peerId);
      count.textContent = String(value.length);
    }),
    actionButton("Attach", () => fileInput.click()),
    element("span", "composer-hint", "Enter to send · Shift+Enter for a new line"),
    count,
    submitButton("message.send"),
  );
  if (uiState.replyTargetEventId && uiState.replyPreview) {
    const replyContext = element("div", "reply-context");
    const replyCopy = element("div", "reply-context-copy");
    replyCopy.append(
      element("strong", "", `Replying to ${uiState.replyPreview.authorName}`),
      element("span", "muted", replyExcerpt(uiState.replyPreview.text)),
    );
    replyContext.append(replyCopy, actionButton("Cancel reply", cancelReply));
    form.append(replyContext);
  }
  form.append(input, fileInput, controls);

  return form;
}

/** @param {import("./shell-contract").ShellSnapshotView} snapshot */
function callMeshView(snapshot) {
  const fragment = document.createDocumentFragment();
  const call = snapshot.home?.call;
  const localPeerId = snapshot.home?.profile.peer_id;
  const joined = Boolean(localPeerId && call?.participants.includes(localPeerId));
  const controls = element("div", "control-row");
  if (joined) {
    controls.append(commandButton("call.leave"));
  } else {
    controls.append(
      commandButton("call.join", { video: false }),
      commandButton("call.join", { video: true }),
    );
    controls.children[0].textContent = "Join with microphone";
    controls.children[0].title = "Ask for microphone access and join this room's direct call";
    controls.children[1].textContent = "Join with camera";
    controls.children[1].title = "Ask for camera and microphone access and join this room's direct call";
  }
  const status = element(
    "p",
    "summary",
    joined
      ? `${call?.participants.length ?? 0} of 4 people in this direct call. Your media does not pass through a Voxelle service.`
      : "Start a direct call for up to four people in this room. Choose whether to turn on your camera before joining.",
  );
  status.setAttribute("aria-live", "polite");
  const videos = element("div", "call-grid");
  if (joined) {
    const localMedia = uiState.localMediaMode === "video"
      ? callVideo("local", true)
      : element("div", "call-media-placeholder", "Microphone on");
    videos.append(callTile(`You · ${uiState.localMediaMode === "video" ? "Camera on" : "Voice only"}`, localMedia));
    for (const peerId of call?.participants ?? []) {
      if (peerId === localPeerId) continue;
      const label = participantConnectionLabel(uiState.peerConnectionStates.get(peerId));
      videos.append(callTile(`${profileForPeer(snapshot, peerId).display_name} · ${label}`, callVideo(peerId, false)));
    }
  }
  fragment.append(status);
  if (uiState.mediaNotice) {
    const notice = element("p", "call-warning", uiState.mediaNotice);
    notice.setAttribute("role", "status");
    notice.setAttribute("aria-live", "polite");
    fragment.append(notice);
  }
  fragment.append(controls, videos);
  return fragment;
}

function callVideo(peerId, muted) {
  const video = element("video", "call-video");
  video.autoplay = true;
  video.muted = muted;
  video.playsInline = true;
  video.dataset.peerId = peerId;
  return video;
}

function callTile(label, video) {
  const tile = element("figure", "call-tile");
  tile.append(video, element("figcaption", "mono", label));
  return tile;
}

function attachCallMedia() {
  const localVideo = app.querySelector('video[data-peer-id="local"]');
  if (localVideo && localVideo.srcObject !== uiState.localMediaStream) {
    localVideo.srcObject = uiState.localMediaStream;
  }
  for (const [peerId, stream] of uiState.remoteMediaStreams) {
    const video = [...app.querySelectorAll("video[data-peer-id]")]
      .find((candidate) => candidate.dataset.peerId === peerId);
    if (video && video.srcObject !== stream) video.srcObject = stream;
  }
}

async function processCallSignals() {
  if (uiState.processingCallSignals || !uiState.localMediaStream || !currentSnapshot.home) return;
  uiState.processingCallSignals = true;
  try {
    const localPeerId = currentSnapshot.home.profile.peer_id;
    const call = currentSnapshot.home.call;
    const retainedSignalIds = new Set(call.signals.map((signal) => signal.event_id));
    for (const eventId of uiState.seenCallSignals) {
      if (!retainedSignalIds.has(eventId)) uiState.seenCallSignals.delete(eventId);
    }
    for (const signal of call.signals) {
      try {
        await consumeRetainedSignal(signal, uiState.seenCallSignals, async () => {
          if (signal.kind === "CALL_JOIN" && signal.author_peer_id !== localPeerId) {
            const pc = ensurePeerConnection(signal.author_peer_id, call.call_id);
            if (localPeerId.localeCompare(signal.author_peer_id) < 0 && pc.signalingState === "stable") {
              const offer = await pc.createOffer();
              await pc.setLocalDescription(offer);
              await sendCallSignal("offer", signal.author_peer_id, JSON.stringify(offer), null);
            }
          } else if (signal.kind === "CALL_LEAVE" && signal.author_peer_id !== localPeerId) {
            closePeerConnection(signal.author_peer_id);
          } else if (signal.target_peer_id === localPeerId && signal.kind === "CALL_OFFER" && signal.sdp) {
            const pc = ensurePeerConnection(signal.author_peer_id, call.call_id);
            await pc.setRemoteDescription(JSON.parse(signal.sdp));
            await flushPendingIce(signal.author_peer_id, pc);
            const answer = await pc.createAnswer();
            await pc.setLocalDescription(answer);
            await sendCallSignal("answer", signal.author_peer_id, JSON.stringify(answer), null);
          } else if (signal.target_peer_id === localPeerId && signal.kind === "CALL_ANSWER" && signal.sdp) {
            const pc = ensurePeerConnection(signal.author_peer_id, call.call_id);
            await pc.setRemoteDescription(JSON.parse(signal.sdp));
            await flushPendingIce(signal.author_peer_id, pc);
          } else if (signal.target_peer_id === localPeerId && signal.kind === "CALL_ICE" && signal.candidate) {
            const pc = ensurePeerConnection(signal.author_peer_id, call.call_id);
            const candidate = JSON.parse(signal.candidate);
            if (pc.remoteDescription) await pc.addIceCandidate(candidate);
            else {
              const pending = uiState.pendingIce.get(signal.author_peer_id) ?? [];
              if (pending.length >= 64) pending.shift();
              pending.push(candidate);
              uiState.pendingIce.set(signal.author_peer_id, pending);
            }
          }
        });
      } catch (error) {
        const detail = errorMessage(error);
        uiState.mediaNotice = `Ignored invalid call signal from ${shortId(signal.author_peer_id)}.`;
        appendActivity(currentSnapshot, `invalid call signal ${signal.event_id}: ${detail}`);
      }
    }
  } finally {
    uiState.processingCallSignals = false;
    attachCallMedia();
  }
}

function ensurePeerConnection(peerId, callId) {
  const existing = uiState.peerConnections.get(peerId);
  if (existing) return existing;
  const pc = new RTCPeerConnection({ iceServers: [] });
  uiState.peerConnectionStates.set(peerId, "connecting");
  for (const track of uiState.localMediaStream?.getTracks() ?? []) {
    pc.addTrack(track, uiState.localMediaStream);
  }
  pc.addEventListener("icecandidate", (event) => {
    if (event.candidate) {
      sendCallSignal("ice", peerId, null, JSON.stringify(event.candidate.toJSON())).catch(reportError);
    }
  });
  pc.addEventListener("track", (event) => {
    let stream = uiState.remoteMediaStreams.get(peerId);
    if (!stream) {
      stream = new MediaStream();
      uiState.remoteMediaStreams.set(peerId, stream);
    }
    stream.addTrack(event.track);
    attachCallMedia();
  });
  pc.addEventListener("connectionstatechange", () => {
    if (uiState.peerConnections.get(peerId) !== pc) return;
    uiState.peerConnectionStates.set(peerId, pc.connectionState);
    if (pc.connectionState === "failed") {
      uiState.mediaNotice = `Could not connect directly to ${profileForPeer(currentSnapshot, peerId).display_name}. They can leave and rejoin to try again.`;
      closePeerConnection(peerId);
      uiState.peerConnectionStates.set(peerId, "failed");
    } else if (pc.connectionState === "closed") {
      closePeerConnection(peerId);
    }
    render();
  });
  pc.__voxelleCallId = callId;
  uiState.peerConnections.set(peerId, pc);
  return pc;
}

async function flushPendingIce(peerId, pc) {
  for (const candidate of uiState.pendingIce.get(peerId) ?? []) {
    await pc.addIceCandidate(candidate);
  }
  uiState.pendingIce.delete(peerId);
}

async function sendCallSignal(signal_type, target_peer_id, sdp, candidate) {
  currentSnapshot = await shell.execute("call.signal", {
    room: currentSnapshot.home?.room.room_id ?? null,
    call_id: currentSnapshot.home?.call?.call_id ?? "",
    target_peer_id,
    signal_type,
    sdp,
    candidate,
  });
  render();
}

function closePeerConnection(peerId) {
  uiState.peerConnections.get(peerId)?.close();
  uiState.peerConnections.delete(peerId);
  uiState.remoteMediaStreams.delete(peerId);
  uiState.pendingIce.delete(peerId);
  uiState.peerConnectionStates.delete(peerId);
}

function stopLocalMedia() {
  for (const track of uiState.localMediaStream?.getTracks() ?? []) track.stop();
  uiState.localMediaStream = null;
  uiState.localMediaMode = "";
  for (const peerId of [...uiState.peerConnections.keys()]) closePeerConnection(peerId);
  uiState.peerConnectionStates.clear();
  uiState.seenCallSignals.clear();
}

function unknownView() {
  return element("p", "summary", "Unknown view");
}

function commandPalette(snapshot) {
  const backdrop = element("div", "command-palette-backdrop");
  backdrop.addEventListener("pointerdown", (event) => {
    if (event.target === backdrop) {
      uiState.paletteOpen = false;
      render();
    }
  });
  const dialog = element("section", "command-palette");
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  dialog.setAttribute("aria-label", "Command Palette");
  const form = element("form", "command-palette-form");
  const input = element("input", "command-palette-input");
  input.type = "search";
  input.placeholder = "Type a command";
  input.value = uiState.paletteQuery;
  input.setAttribute("aria-label", "Search commands");
  const list = element("ol", "command-palette-list");
  const updateResults = () => {
    uiState.paletteQuery = input.value;
    populatePaletteResults(list, snapshot);
  };
  input.addEventListener("input", updateResults);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const first = filterPaletteCommands(
      snapshot.ui_ontology.commands,
      uiState.paletteQuery,
    )[0];
    if (first) executePaletteCommand(first.id);
  });
  form.append(input);
  dialog.append(form);
  populatePaletteResults(list, snapshot);
  dialog.append(list);
  backdrop.append(dialog);
  return backdrop;
}

function populatePaletteResults(list, snapshot) {
  list.replaceChildren();
  const commands = filterPaletteCommands(
    snapshot.ui_ontology.commands,
    uiState.paletteQuery,
  );
  if (commands.length === 0) {
    list.append(element("li", "command-palette-empty", "No matching commands"));
  }
  for (const command of commands) {
    const item = element("li", "");
    const button = element("button", "command-palette-result");
    button.type = "button";
    const copy = element("span", "command-palette-copy");
    copy.append(
      element("strong", "", command.label),
      element("small", "muted", command.description),
    );
    button.append(copy);
    if (command.shortcut) {
      button.append(element("kbd", "", command.shortcut.replace("Mod", navigator.platform.includes("Mac") ? "⌘" : "Ctrl")));
    }
    button.addEventListener("click", () => executePaletteCommand(command.id));
    item.append(button);
    list.append(item);
  }
}

function executePaletteCommand(commandId) {
  uiState.paletteOpen = false;
  uiState.paletteQuery = "";
  runCommand(commandId).catch(reportError);
}

/**
 * @param {Array<[string, string]>} rows
 */
function definitionGrid(rows) {
  const grid = element("dl", "definition-grid");
  for (const [term, value] of rows) {
    grid.append(element("dt", "", term), element("dd", "mono", value));
  }
  return grid;
}

/**
 * @param {string} command
 */
function commandLabel(command) {
  return currentSnapshot.ui_ontology.commands.find((item) => item.id === command)?.label
    ?? command;
}

/**
 * @param {string} command
 * @param {unknown} [payload]
 */
function commandButton(command, payload) {
  const button = element("button", "command-button", commandLabel(command));
  button.type = "button";
  button.dataset.command = command;
  button.dataset.actionKey = `command:${command}:${JSON.stringify(payload ?? null)}`;
  const definition = currentSnapshot.ui_ontology.commands.find((item) => item.id === command);
  if (definition?.shortcut) {
    button.title = `${definition.description} (${definition.shortcut})`;
  }
  button.disabled = uiState.busyCommand !== "";
  if (uiState.busyCommand === command) {
    button.textContent = "Working";
  }
  button.addEventListener("click", () => {
    runCommand(command, payload).catch(reportError);
  });
  return button;
}

function actionButton(label, action, title = label) {
  const button = element("button", "command-button", label);
  button.type = "button";
  button.title = title;
  button.dataset.actionKey = `action:${label}`;
  button.disabled = uiState.busyCommand !== "";
  button.addEventListener("pointerdown", (event) => event.stopPropagation());
  button.addEventListener("click", action);
  return button;
}

async function saveLayout(placements) {
  await runCommand("workbench.layout.save", { placements });
}

/** @param {string} command */
function submitButton(command) {
  const button = element("button", "command-button", commandLabel(command));
  button.type = "submit";
  button.disabled = uiState.busyCommand !== "";
  return button;
}

/**
 * @param {string} command
 * @param {unknown} [payload]
 */
async function runCommand(command, payload) {
  const commandReturnElement = focusCoordinator.currentElement();
  // Capture before the busy render disables the focused command button; browsers
  // may blur a control as soon as it becomes disabled.
  if (command === "workbench.commandPalette.open") rememberFocusReturn();
  uiState.busyCommand = command;
  uiState.status = "";
  clearError();
  render();
  try {
    switch (command) {
      case "workbench.commandPalette.open":
        uiState.connectionOpen = false;
        uiState.paletteOpen = true;
        uiState.paletteQuery = "";
        return;
      case "message.composer.focus":
        uiState.paletteOpen = false;
        window.requestAnimationFrame(() => {
          app.querySelector(".message-input")?.focus();
        });
        return;
      case "shell.refresh":
        await refresh();
        return;
      case "home.init":
        currentSnapshot = await shell.execute(command, { default_room: null });
        return;
      case "home.archiveForRecovery":
        currentSnapshot = await shell.execute(command);
        uiState.preparingHomeRecovery = false;
        uiState.homeRecoveryNotice = "Unusable local state was archived without deletion. Choose Recover My Identity and open your offline recovery kit.";
        return;
      case "runtime.goOnline":
        currentSnapshot = await shell.execute(command, {
          bind: blankToNull(uiState.bindDraft),
          advertise: blankToNull(uiState.advertiseDraft),
        });
        return;
      case "runtime.goOffline":
        currentSnapshot = await shell.execute(command);
        return;
      case "space.invite.create":
        currentSnapshot = await shell.execute(command, { expires_minutes: 1440 });
        return;
      case "space.join":
        currentSnapshot = await shell.execute(command, {
          space_invite_json: uiState.spaceInviteDraft,
          max_events: 4096,
        });
        uiState.spaceInviteDraft = "";
        return;
      case "identity.recovery.export": {
        if (!currentSnapshot.home) {
          throw new Error("Create or join a space before saving a recovery kit.");
        }
        const path = payload?.path ?? await shell.chooseRecoveryKitPath?.("save");
        if (!path) return;
        currentSnapshot = await shell.execute(command, { path });
        return;
      }
      case "identity.recovery.restore": {
        if (currentSnapshot.home) {
          throw new Error("Identity recovery requires a fresh Voxelle home.");
        }
        const path = payload?.path ?? await shell.chooseRecoveryKitPath?.("open");
        if (!path) return;
        currentSnapshot = await shell.execute(command, {
          path,
          max_events_per_peer: payload?.max_events_per_peer ?? 4096,
        });
        return;
      }
      case "peer.import":
        currentSnapshot = await shell.execute(command, {
          peer_record_json: uiState.peerRecordDraft,
        });
        uiState.peerRecordDraft = "";
        if (ontologyPresentation(currentSnapshot.ui_ontology).syncAutoAfterImport) {
          currentSnapshot = await shell.execute("peer.sync", firstPeerRequest());
        }
        return;
      case "peer.diagnose":
      case "peer.sync":
        currentSnapshot = await shell.execute(
          command,
          /** @type {import("./shell-contract").PeerCommandRequest} */ (
            payload ?? firstPeerRequest()
          ),
        );
        return;
      case "message.send": {
        const text = payload?.text ?? uiState.messageDraft;
        currentSnapshot = await shell.execute(command, {
          text,
          room: payload?.room ?? null,
          mentions: payload?.mentions
            ?? mentionedPeerIds(
              text,
              currentSnapshot.home?.profiles ?? [],
              uiState.messageMentionsDraft,
            ),
          thread_root_event_id: payload?.thread_root_event_id ?? blankToNull(uiState.replyTargetEventId),
        });
        uiState.messageDraft = "";
        uiState.messageMentionsDraft.clear();
        uiState.replyTargetEventId = "";
        uiState.replyPreview = null;
        return;
      }
      case "channel.select":
        currentSnapshot = await shell.execute(command, payload);
        return;
      case "message.open":
        currentSnapshot = await shell.execute(command, payload);
        return;
      case "channel.markRead":
        currentSnapshot = await shell.execute(command, payload ?? { room_id: null });
        return;
      case "channel.rotateKey":
        currentSnapshot = await shell.execute(command, payload);
        return;
      case "channel.create": {
        const privateMembers = uiState.channelPrivateDraft
          ? [...uiState.channelMembersDraft]
          : [];
        if (uiState.channelPrivateDraft && privateMembers.length === 0) {
          const ownPeerId = currentSnapshot.home?.profile.peer_id;
          if (ownPeerId) privateMembers.push(ownPeerId);
        }
        currentSnapshot = await shell.execute(command, payload ?? {
          name: uiState.channelNameDraft,
          topic: uiState.channelTopicDraft,
          private_members: privateMembers,
        });
        uiState.channelNameDraft = "";
        uiState.channelTopicDraft = "";
        uiState.channelPrivateDraft = false;
        uiState.channelMembersDraft.clear();
        return;
      }
      case "message.edit": {
        if (!payload?.target_event_id || typeof payload?.text !== "string") {
          throw new Error("Choose Edit from one of your messages first.");
        }
        currentSnapshot = await shell.execute(command, {
          ...payload,
          mentions: payload.mentions
            ?? mentionedPeerIds(
              payload.text,
              currentSnapshot.home?.profiles ?? [],
              uiState.messageEditMentionsDraft,
            ),
        });
        uiState.editingMessageId = "";
        uiState.messageEditDraft = "";
        uiState.messageEditMentionsDraft.clear();
        return;
      }
      case "message.redact":
        currentSnapshot = await shell.execute(command, payload);
        uiState.deletingMessageId = "";
        return;
      case "reaction.add":
      case "reaction.remove":
      case "pin.add":
      case "pin.remove":
      case "attachment.add":
        currentSnapshot = await shell.execute(command, payload);
        return;
      case "profile.update":
        currentSnapshot = await shell.execute(command, payload ?? {
          display_name: uiState.profileNameDraft,
          about: uiState.profileAboutDraft,
        });
        uiState.profileNameDraft = "";
        uiState.profileAboutDraft = "";
        uiState.profileDraftInitialized = false;
        return;
      case "message.search":
        currentSnapshot = await shell.execute(command, payload ?? {
          query: uiState.searchDraft,
          room: null,
          limit: 50,
        });
        return;
      case "call.join": {
        if (!navigator.mediaDevices?.getUserMedia || typeof RTCPeerConnection === "undefined") {
          uiState.mediaNotice = "Voice and video are unavailable in this installation. Update Voxelle or use a supported native build, then try again.";
          return;
        }
        stopLocalMedia();
        let capture;
        try {
          capture = await captureCallMedia(navigator.mediaDevices, Boolean(payload?.video));
        } catch (error) {
          uiState.mediaNotice = mediaCaptureErrorMessage(error, Boolean(payload?.video));
          return;
        }
        uiState.localMediaStream = capture.stream;
        uiState.localMediaMode = capture.video ? "video" : "voice";
        uiState.mediaNotice = capture.notice;
        try {
          currentSnapshot = await shell.execute(command, {
            room: currentSnapshot.home?.room.room_id ?? null,
            video: capture.video,
          });
          uiState.lastCallHeartbeatMs = Date.now();
        } catch (error) {
          stopLocalMedia();
          throw error;
        }
        return;
      }
      case "call.leave":
        currentSnapshot = await leaveCall(
          () => shell.execute(command, {
            room: currentSnapshot.home?.room.room_id ?? null,
            call_id: currentSnapshot.home?.call?.call_id ?? "",
          }),
          () => {
            stopLocalMedia();
            uiState.mediaNotice = null;
            uiState.lastCallHeartbeatMs = 0;
          },
        );
        return;
      case "call.heartbeat":
        currentSnapshot = await shell.execute(command, payload);
        uiState.lastCallHeartbeatMs = Date.now();
        return;
      case "call.signal":
        currentSnapshot = await shell.execute(command, payload);
        return;
      case "role.create": {
        if (!payload) {
          uiState.roleCreateOpen = true;
          openPeopleUtility();
          return;
        }
        currentSnapshot = await shell.execute(command, {
          name: payload.name,
          permissions: payload.permissions,
        });
        uiState.roleNameDraft = "";
        uiState.rolePermissionsDraft.clear();
        uiState.roleCreateOpen = false;
        return;
      }
      case "role.grant":
      case "role.revoke": {
        if (!payload?.peer_id || !payload?.role_id) {
          openPeopleUtility();
          return;
        }
        currentSnapshot = await shell.execute(command, payload);
        uiState.roleAssignmentDraft = null;
        focusRoleRow(payload.role_id);
        return;
      }
      case "member.ban":
      case "member.unban": {
        if (!payload?.peer_id) {
          openPeopleUtility();
          return;
        }
        currentSnapshot = await shell.execute(command, payload);
        uiState.banningPeerId = "";
        focusProfileRow(payload.peer_id);
        return;
      }
      case "invite.copy":
        if (shell.mode === "preview") {
          throw new Error(
            "Preview only; launch the desktop app to copy a usable invite.",
          );
        }
        if (!currentSnapshot.home?.invite?.space_invite_json) {
          throw new Error("Create a signed invite before copying it.");
        }
        await copyTextToClipboard(
          navigator.clipboard,
          currentSnapshot.home?.invite?.space_invite_json ?? "",
        );
        uiState.status = "Signed invite copied. Send it privately to the person you want to invite.";
        appendActivity(currentSnapshot, "copied invite");
        return;
      case "ui.preference.set":
        currentSnapshot = await shell.execute(
          command,
          /** @type {import("./shell-contract").SetUiPreferenceRequest} */ (payload),
        );
        return;
      case "ui.preferences.reset":
        currentSnapshot = await shell.execute(command, {});
        return;
      case "workbench.layout.save":
        currentSnapshot = await shell.execute(
          command,
          /** @type {import("./shell-contract").SetWorkbenchLayoutRequest} */ (payload),
        );
        return;
      case "workbench.layout.reset":
        currentSnapshot = await shell.execute(command, {});
        return;
      case "product.update.install":
        currentSnapshot = await shell.execute(command, {
          package_json: uiState.productUpdateDraft,
        });
        uiState.productUpdateDraft = "";
        return;
      case "product.update.rotateTrust":
        currentSnapshot = await shell.execute(command, {
          transition_json: uiState.trustTransitionDraft,
        });
        uiState.trustTransitionDraft = "";
        return;
      case "product.update.check":
      case "product.update.stageAvailable":
      case "product.update.activateStaged":
      case "product.update.discardStaged":
      case "product.update.rollback":
        currentSnapshot = await shell.execute(command, {});
        return;
      default:
        throw new Error(`No command handler is registered for ${command}`);
    }
  } finally {
    uiState.busyCommand = "";
    render();
    focusCoordinator.restoreWhenNoSurface(
      commandReturnElement,
      () => commandCompletionFocusTarget(command),
    );
    if (refreshQueued) queueMicrotask(() => publishRefresh().catch(reportError));
  }
}

function commandCompletionFocusTarget(command) {
  if (command === "home.archiveForRecovery") {
    return app.querySelector('[data-command="identity.recovery.restore"]');
  }
  if (command === "home.init" || command === "space.join") {
    return app.querySelector(".recovery-setup-prompt .command-button")
      ?? app.querySelector(".message-input");
  }
  if (
    command === "identity.recovery.export"
    || command === "identity.recovery.restore"
    || command === "channel.create"
  ) {
    return app.querySelector(".message-input");
  }
  return null;
}

/** @param {unknown} error */
function reportError(error) {
  uiState.busyCommand = "";
  const presentation = presentShellError(error);
  uiState.error = presentation.message;
  uiState.errorRecovery = presentation.recoveryMessage;
  uiState.errorDetail = presentation.detail;
  appendActivity(currentSnapshot, `error (${presentation.recovery}): ${presentation.message}`);
  render();
}

function setUserError(message) {
  uiState.error = message;
  uiState.errorRecovery = "Correct the highlighted information and try again.";
  uiState.errorDetail = "";
}

function clearError() {
  uiState.error = "";
  uiState.errorRecovery = "";
  uiState.errorDetail = "";
}

/** @param {unknown} error */
function errorMessage(error) {
  if (error instanceof Error) {
    return error.message;
  }
  if (error && typeof error === "object" && "message" in error) {
    return String(error.message);
  }
  return String(error);
}

function globalErrorBanner() {
  if (!uiState.error) {
    return document.createDocumentFragment();
  }
  const banner = element("section", "error-banner");
  banner.setAttribute("role", "alert");
  const copy = element("div", "error-copy");
  copy.append(
    element("strong", "", uiState.error),
    ...(uiState.errorRecovery ? [element("p", "summary", uiState.errorRecovery)] : []),
  );
  if (uiState.errorDetail) {
    const details = element("details", "advanced-details");
    details.append(
      element("summary", "", "Technical details"),
      element("pre", "error-detail", uiState.errorDetail),
    );
    copy.append(details);
  }
  banner.append(
    copy,
    actionButton("Dismiss", () => {
      clearError();
      render();
    }),
  );
  return banner;
}

function globalStatusBanner() {
  if (!uiState.status) {
    return document.createDocumentFragment();
  }
  const banner = element("section", "status-banner");
  banner.setAttribute("role", "status");
  banner.setAttribute("aria-live", "polite");
  banner.append(
    element("strong", "", uiState.status),
    actionButton("Dismiss", () => {
      uiState.status = "";
      render();
      window.requestAnimationFrame(() => app.querySelector('[data-command="invite.copy"]')?.focus());
    }),
  );
  return banner;
}

function rememberFocusReturn() {
  focusCoordinator.rememberReturnElement();
}

function synchronizeTransientFocus() {
  const surface = uiState.paletteOpen
    ? "palette"
    : uiState.connectionOpen
      ? "connection"
      : uiState.utilityOpen
        ? `utility:${uiState.utilityOpen}`
        : "";
  focusCoordinator.synchronize(surface, () => {
    const target = surface === "palette"
      ? app.querySelector(".command-palette-input")
      : app.querySelector("[data-dialog-initial-focus='true']");
    if (surface === "palette") {
      target?.setSelectionRange?.(target.value.length, target.value.length);
    }
    return target;
  });
}

function firstPeerRequest() {
  const peer = currentSnapshot.home?.peers[0];
  if (!peer) {
    throw new Error("no peer available");
  }
  return peerRequest(peer);
}

/** @param {import("./shell-contract").PeerListItemView} peer */
function peerRequest(peer) {
  return {
    peer_id: peer.peer_id,
    device_id: peer.device_id,
    max_events: 64,
  };
}

/** @param {import("./shell-contract").NetworkHealthStatus} status */
function statusLabel(status) {
  return connectionHealthLabel(status);
}

/**
 * @param {string} label
 * @param {string} placeholder
 * @param {string} value
 * @param {(value: string) => void} onInput
 */
function labeledInput(label, placeholder, value, onInput) {
  const field = element("label", "field");
  const input = element("input", "");
  input.placeholder = placeholder;
  input.value = value;
  input.addEventListener("input", () => {
    onInput(input.value);
  });
  field.append(element("span", "", label), input);
  return field;
}

function choiceCheckbox(label, checked, onChange) {
  const field = element("label", "choice-checkbox");
  const input = element("input", "");
  input.type = "checkbox";
  input.checked = checked;
  input.addEventListener("change", () => onChange(input.checked));
  field.append(input, element("span", "", label));
  return field;
}

function mentionPicker(input, snapshot, onChange) {
  const details = element("details", "mention-picker");
  details.append(element("summary", "command-button", "Mention someone"));
  const choices = element("div", "mention-choices");
  const profiles = (snapshot.home?.profiles ?? []).filter((profile) =>
    !profile.banned && profile.peer_id !== snapshot.home?.profile.peer_id);
  const nameCounts = new Map();
  for (const profile of profiles) {
    const name = profile.display_name.trim().toLocaleLowerCase();
    nameCounts.set(name, (nameCounts.get(name) ?? 0) + 1);
  }
  if (profiles.length === 0) {
    choices.append(element("p", "summary", "No other current members to mention."));
  }
  for (const profile of profiles) {
    const duplicate = nameCounts.get(profile.display_name.trim().toLocaleLowerCase()) > 1;
    const label = duplicate
      ? `Mention ${profile.display_name} · member ${shortId(profile.peer_id)}`
      : `Mention ${profile.display_name}`;
    choices.append(actionButton(label, () => {
      const insertion = insertMentionText(
        input.value,
        input.selectionStart,
        input.selectionEnd,
        profile.display_name,
      );
      input.value = insertion.text;
      onChange(insertion.text, profile.peer_id);
      details.open = false;
      input.focus();
      input.setSelectionRange(insertion.caret, insertion.caret);
    }));
  }
  details.append(choices);
  return details;
}

function updateSet(values, value, included) {
  if (included) values.add(value);
  else values.delete(value);
}

function permissionLabel(permission) {
  return {
    "message:post": "Post messages",
    "message:moderate": "Edit or remove other people's messages",
    "message:pin": "Pin messages",
    "channel:manage": "Create and manage channels",
    "role:manage": "Create roles and assign access",
    "member:ban": "Ban members or allow rejoining",
    "invite:create": "Invite people",
  }[permission] ?? permission;
}

function openPeopleUtility() {
  uiState.connectionOpen = false;
  uiState.paletteOpen = false;
  uiState.utilityOpen = "people";
}

/** @param {string} value */
function blankToNull(value) {
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}

function replyExcerpt(text) {
  const compact = text.replace(/\s+/g, " ").trim();
  return compact.length > 80 ? `${compact.slice(0, 77)}…` : compact;
}

/** @param {File} file */
async function fileAsBase64(file) {
  const dataUrl = await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener("load", () => resolve(String(reader.result)));
    reader.addEventListener("error", () => reject(reader.error ?? new Error("file read failed")));
    reader.readAsDataURL(file);
  });
  return String(dataUrl).split(",", 2)[1] ?? "";
}

/**
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 * @param {string} text
 */
function activityIncludes(snapshot, text) {
  return snapshot.service_activity.some((item) => item.summary.includes(text));
}

/**
 * @param {import("./shell-contract").ShellSnapshotView} snapshot
 * @param {string} summary
 */
function appendActivity(snapshot, summary) {
  const id = snapshot.service_activity.at(-1)?.id ?? 0;
  snapshot.service_activity.push({ id: id + 1, level: "info", summary });
}

/** @param {string} text */
function shortId(text) {
  const value = text.startsWith("ed25519:") ? text.slice(8) : text;
  return value.length > 12 ? `${value.slice(0, 12)}` : value;
}

function profileForPeer(snapshot, peerId) {
  return snapshot.home?.profiles.find((profile) => profile.peer_id === peerId) ?? {
    peer_id: peerId,
    display_name: `Member ${shortId(peerId)}`,
    about: "",
  };
}

function channelName(snapshot, roomId) {
  const channel = snapshot.home?.channels.find((candidate) => candidate.room_id === roomId);
  return channel ? `# ${channel.name}` : roomId;
}

function profileInitials(displayName) {
  const parts = displayName.trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "?";
  return parts.slice(0, 2).map((part) => part[0].toUpperCase()).join("");
}

/**
 * @param {keyof HTMLElementTagNameMap} tag
 * @param {string} className
 * @param {string} [text]
 */
function element(tag, className, text) {
  const el = document.createElement(tag);
  if (className) {
    el.className = className;
  }
  if (text !== undefined) {
    el.textContent = text;
  }
  return el;
}

return async function disposeProductComponent() {
  window.clearInterval(heartbeatTimer);
  document.removeEventListener("keydown", handleKeydown);
  stopSnapshotInvalidation?.();
  stopLocalMedia();
  for (const peerId of [...uiState.peerConnections.keys()]) {
    closePeerConnection(peerId);
  }
  app.replaceChildren();
};
