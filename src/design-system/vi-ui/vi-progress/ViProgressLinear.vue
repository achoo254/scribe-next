<script setup lang="ts">
// ViProgressLinear — wrapper iNET ViUI quanh v-progress-linear. Thanh tiến độ (Mode B cắt file).
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-progress-linear v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-progress-linear>
</template>
