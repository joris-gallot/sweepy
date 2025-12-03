export const api = {
  get: (url: string) => fetch(url),
  post: (url: string, data: unknown) => fetch(url, { method: 'POST', body: JSON.stringify(data) })
};

export const config = {
  baseUrl: 'https://api.example.com',
  timeout: 5000
};

export const unusedApiFunction = () => {
  return 'This function is never used';
};

export type ApiConfig = typeof config;
