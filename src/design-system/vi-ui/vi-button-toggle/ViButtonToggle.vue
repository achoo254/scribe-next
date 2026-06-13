<script setup lang="ts">
// ViButtonToggle — wrapper iNET ViUI quanh v-btn-toggle. Toggle nhóm (vd phút/MB, format).
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-btn-toggle v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-btn-toggle>
</template>
