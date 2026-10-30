import { redirect } from "next/navigation";

// Prerendered as a static redirect to the frames page.
export default function Home() {
  redirect("/frames");
}
