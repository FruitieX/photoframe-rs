"use client";
import { useConfigQuery } from "../../hooks/http";
import Container from "@mui/material/Container";
import Grid from "@mui/material/Grid";
import { FrameCard, Frame } from "../../components/FrameCard";
import { Skeleton } from "@mui/material";

const API_BASE =
  process.env.NEXT_PUBLIC_API_BASE ??
  (typeof window !== "undefined" ? "" : "http://localhost:8080");

export default function FramesPage() {
  const { data, refetch, isLoading, error } = useConfigQuery(API_BASE);
  const frames: Frame[] = data
    ? Object.entries(data.photoframes)
        .map(([id, f]) => ({ id, ...f }))
        .sort((a, b) => a.id.localeCompare(b.id))
    : [];
  return (
    <Container className="py-6">
      <h1 className="text-xl font-semibold mb-4">Photo Frames</h1>
      {error && <p className="text-red-600">Error</p>}
      <Grid container spacing={2}>
        {isLoading && (
          <>
            <Skeleton variant="rounded" width="100%" height={800} />
            <Skeleton variant="rounded" width="100%" height={1000} />
          </>
        )}
        {frames.map((f) => (
          <Grid key={f.id} size={12}>
            <FrameCard frame={f} refresh={() => refetch()} apiBase={API_BASE} />
          </Grid>
        ))}
      </Grid>
    </Container>
  );
}
