<script setup lang="ts">
// ViSwitch — wrapper iNET ViUI quanh v-switch. Toggle bật/tắt (vd "Tự động transcript khi ghi").
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-switch color="primary" density="comfortable" hide-details v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-switch>
</template>
