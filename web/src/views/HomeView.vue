<script setup lang="ts">
import { useRouter } from "vue-router";
import { ViCard, ViButton } from "@hobui/viui";
import AppTopBar from "../components/AppTopBar.vue";

const router = useRouter();

function goSplit(): void {
  void router.push({ name: "split" });
}
</script>

<template>
  <div>
    <AppTopBar />
    <main class="content">
      <ViCard rounded="lg" class="hero">
        <h1 class="text-h5 font-weight-bold mb-2">Chào mừng đến OneScribe</h1>
        <p class="text-body-2 text-medium-emphasis mb-6">
          Ghi &amp; cắt media. Chức năng ghi sẽ bổ sung ở các phase sau.
        </p>
        <ViButton color="primary" variant="flat" size="large" label="Cắt file" @click="goSplit" />
      </ViCard>
    </main>
  </div>
</template>

<style scoped>
.content {
  max-width: 640px;
  margin: 0 auto;
  padding: var(--space-8pt-4) var(--space-8pt-3);
}
.hero {
  padding: var(--space-8pt-4);
}
</style>
