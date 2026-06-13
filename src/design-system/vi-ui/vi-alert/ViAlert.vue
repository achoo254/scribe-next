<script setup lang="ts">
// ViAlert — wrapper iNET ViUI quanh v-alert (spec: ViUI MCP get_component_api/ViAlert).
// Dùng đúng type: success|warning|error|info (màu đã tokenize). Hỗ trợ closable.
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-alert v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-alert>
</template>
