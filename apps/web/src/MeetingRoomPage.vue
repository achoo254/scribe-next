<script setup lang="ts">
// Web client (Phase 4) — khách join bằng trình duyệt: nhập tên → lấy token guest (token-service) →
// connect LiveKit → xem video + đọc phụ đề realtime. Consumer THUẦN: KHÔNG host, KHÔNG transcribe.
import { ref } from "vue";
import {
  useLiveKitViewer,
  useCaptionFeed,
  fetchGuestToken,
  fetchCaptionHistory,
} from "@onescribe/meeting-shared";
import VideoTile from "./components/VideoTile.vue";

const serverUrl = ref(import.meta.env.VITE_MEETING_SERVER || "http://localhost:8787");
const room = ref("");
const name = ref("");
const joining = ref(false);
const joinError = ref("");

const lk = useLiveKitViewer();
const feed = useCaptionFeed();

async function join() {
  if (!name.value.trim() || !room.value.trim()) {
    joinError.value = "Nhập tên và mã phòng.";
    return;
  }
  joinError.value = "";
  joining.value = true;
  try {
    const t = await fetchGuestToken(serverUrl.value.trim(), room.value.trim(), name.value.trim());
    const r = await lk.connect(t.url, t.token, t.turn);
    if (r) {
      feed.bind(r);
      // Người vào trễ: nạp lịch sử caption trong TTL rồi nghe realtime tiếp.
      feed.seedHistory(await fetchCaptionHistory(serverUrl.value.trim(), room.value.trim(), t.token));
    } else {
      joinError.value = lk.error.value || "Không kết nối được phòng.";
    }
  } catch (e) {
    joinError.value = e instanceof Error ? e.message : String(e);
  } finally {
    joining.value = false;
  }
}

async function leave() {
  const r = lk.room.value;
  if (r) feed.unbind(r);
  await lk.disconnect();
}

const STATUS_TEXT: Record<string, string> = {
  transcribe_down: "Phụ đề tạm dừng (host đang khởi động lại)",
  mic_lost: "Mic host bị thu hồi — phụ đề tạm dừng",
  overloaded: "Máy host quá tải — phụ đề có thể trễ",
};
</script>

<template>
  <div class="page">
    <!-- Join form -->
    <div v-if="lk.state.value !== 'connected'" class="join">
      <h1 class="join__title">OneScribe — Phòng họp</h1>
      <p class="join__sub">Khách tham gia: xem video &amp; đọc phụ đề trực tiếp.</p>
      <label>Máy chủ token<input v-model="serverUrl" :disabled="joining" /></label>
      <label>Mã phòng<input v-model="room" placeholder="vd: UUID phòng" :disabled="joining" /></label>
      <label>Tên của bạn<input v-model="name" placeholder="Tên hiển thị" :disabled="joining" /></label>
      <button class="primary" :disabled="joining" @click="join">
        {{ joining ? "Đang vào…" : "Tham gia" }}
      </button>
      <p v-if="joinError" class="join__err">{{ joinError }}</p>
    </div>

    <!-- In room -->
    <div v-else class="room">
      <div class="room__stage">
        <div class="room__grid">
          <VideoTile v-for="p in lk.participants.value" :key="p.identity" :p="p" />
        </div>
        <aside class="room__captions">
          <div class="cap__head">Phụ đề trực tiếp</div>
          <div v-if="feed.hostStatus.value !== 'ok'" class="cap__status">
            {{ STATUS_TEXT[feed.hostStatus.value] }}
          </div>
          <div class="cap__list">
            <p v-if="!feed.captions.value.length" class="cap__empty">Chưa có phụ đề…</p>
            <div v-for="c in feed.captions.value" :key="c.id" class="cap__line" :class="{ 'cap__line--partial': !c.final }">
              <span class="cap__name">{{ c.name }}</span>{{ c.text }}
            </div>
          </div>
        </aside>
      </div>
      <div class="room__controls">
        <button @click="lk.toggleMic">{{ lk.micOn.value ? "Tắt mic" : "Bật mic" }}</button>
        <button @click="lk.toggleCamera">{{ lk.camOn.value ? "Tắt cam" : "Bật cam" }}</button>
        <button class="leave" @click="leave">Rời phòng</button>
      </div>
      <p v-if="lk.error.value" class="join__err">{{ lk.error.value }}</p>
    </div>
  </div>
</template>

<style scoped>
.page {
  height: 100%;
  padding: var(--sp-md);
}
.join {
  max-width: 400px;
  margin: 8vh auto;
  display: flex;
  flex-direction: column;
  gap: var(--sp-sm);
}
.join__title {
  color: var(--one-green);
  margin: 0;
}
.join__sub {
  opacity: 0.7;
  margin: 0 0 var(--sp-sm);
}
.join label {
  display: flex;
  flex-direction: column;
  gap: var(--sp-xs);
  font-size: 13px;
}
.join__err {
  color: var(--one-error);
}
.room {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: var(--sp-md);
}
.room__stage {
  flex: 1;
  min-height: 0;
  display: flex;
  gap: var(--sp-md);
}
.room__grid {
  flex: 1;
  min-width: 0;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: var(--sp-sm);
  align-content: start;
}
.room__captions {
  width: 320px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: var(--sp-xs);
  border-left: 1px solid var(--one-bg-border);
  padding-left: var(--sp-sm);
}
.cap__head {
  font-weight: 600;
}
.cap__status {
  font-size: 13px;
  color: var(--one-warning);
  background: rgba(247, 173, 48, 0.12);
  padding: var(--sp-xs) var(--sp-sm);
  border-radius: var(--radius);
}
.cap__list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--sp-xs);
}
.cap__empty {
  opacity: 0.5;
}
.cap__line {
  font-size: 13px;
  line-height: 1.4;
}
.cap__line--partial {
  opacity: 0.6;
  font-style: italic;
}
.cap__name {
  font-weight: 600;
  color: var(--one-green);
  margin-right: var(--sp-xs);
}
.room__controls {
  display: flex;
  gap: var(--sp-sm);
  justify-content: center;
}
.room__controls .leave {
  background: var(--one-error);
  color: #fff;
  border-color: var(--one-error);
}
@media (max-width: 720px) {
  .room__stage {
    flex-direction: column;
  }
  .room__captions {
    width: auto;
    border-left: none;
    border-top: 1px solid var(--one-bg-border);
    padding-left: 0;
    padding-top: var(--sp-sm);
    max-height: 180px;
  }
}
</style>
