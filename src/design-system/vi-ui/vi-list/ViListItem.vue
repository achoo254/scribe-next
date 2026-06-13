<script setup lang="ts">
// ViListItem — wrapper iNET ViUI quanh v-list-item. Một dòng trong ViList.
defineOptions({ inheritAttrs: false });
</script>

<template>
  <v-list-item v-bind="$attrs">
    <template v-for="(_, name) in $slots" #[name]="slotProps">
      <slot :name="name" v-bind="slotProps ?? {}" />
    </template>
  </v-list-item>
</template>
