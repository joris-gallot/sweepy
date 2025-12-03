<template>
  <div>Child Component</div>
</template>

<script setup>
import { api } from './api.ts';

export const ChildComponent = {
  name: 'Child'
};

export const useChild = () => {
  return { api };
};

export const unusedChildExport = 'not used';
</script>
