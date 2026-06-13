<script setup lang="ts">
// ViInput — wrapper iNET ViUI quanh v-text-field (spec: ViUI MCP get_component_api/ViInput).
// Viền radius 4px, focus ring primary, lỗi dùng màu error (theo theme). v-model + rules passthrough.
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-text-field variant="outlined" density="comfortable" v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-text-field>
</template>
