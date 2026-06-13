<script setup lang="ts">
// ViSelect — wrapper iNET ViUI quanh v-select (spec: ViUI MCP). Dropdown chọn 1/nhiều.
// Dùng cho: chọn mic, chọn format. v-model + items passthrough qua $attrs.
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-select variant="outlined" density="comfortable" v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-select>
</template>
