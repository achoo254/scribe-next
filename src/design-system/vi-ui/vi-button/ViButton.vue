<script setup lang="ts">
// ViButton — wrapper iNET ViUI quanh v-btn (spec: ViUI MCP get_component_api/ViButton).
// color: primary(One green)|secondary(orange)|success|warning|error|info · variant: elevated|flat|outlined|text|tonal.
// Không hardcode màu — dùng color semantic của theme inetOneDark.
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-btn v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-btn>
</template>
