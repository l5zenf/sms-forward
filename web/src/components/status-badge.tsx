import { Badge, badgeVariants } from "@/components/ui/badge";
import type { VariantProps } from "class-variance-authority";
import { statusBadgeVariant, statusLabel } from "@/utils";
import { cn } from "@/lib/utils";

type BadgeVariant = VariantProps<typeof badgeVariants>["variant"];

/** Tiny colored status pill used uniformly across the dashboard / list / drawer. */
export function StatusBadge({
  status,
  className,
}: {
  status: string;
  className?: string;
}) {
  const variant = statusBadgeVariant(status) as BadgeVariant;
  return (
    <Badge variant={variant} className={cn("shrink-0", className)}>
      {statusLabel(status)}
    </Badge>
  );
}
