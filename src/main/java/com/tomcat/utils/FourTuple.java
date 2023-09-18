package com.tomcat.utils;

public class FourTuple<A, B, C, D> extends ThreeTuple<A, B, C> {
    private final D fourth;
 
    public FourTuple(A a, B b, C c, D d) {
        super(a, b, c);
        this.fourth = d;
    }
 
    public D getFourth() {
        return this.fourth;
    }
}