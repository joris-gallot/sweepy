export const helper = (value: string) => {
  return value.toUpperCase();
};

export const formatDate = (date: Date) => {
  return date.toISOString();
};

export const unusedUtilFunction = () => {
  return 'never used';
};

export type ApiResponse = {
  data: unknown;
  status: number;
};

export interface User {
  id: number;
  name: string;
}
