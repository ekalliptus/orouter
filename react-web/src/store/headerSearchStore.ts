// Ported from src/store/headerSearchStore.js. Pages register a placeholder on
// mount, read the query, unregister on unmount. The Header renders the input
// only while `visible` is true.
import { create } from "zustand";

interface HeaderSearchState {
  query: string;
  placeholder: string;
  visible: boolean;
  setQuery: (query: string) => void;
  register: (placeholder?: string) => void;
  unregister: () => void;
}

export const useHeaderSearchStore = create<HeaderSearchState>((set) => ({
  query: "",
  placeholder: "",
  visible: false,
  setQuery: (query) => set({ query }),
  register: (placeholder = "Search...") => set({ visible: true, placeholder, query: "" }),
  unregister: () => set({ visible: false, placeholder: "", query: "" }),
}));
