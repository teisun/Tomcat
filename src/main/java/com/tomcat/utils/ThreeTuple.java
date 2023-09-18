package com.tomcat.utils;

public class ThreeTuple<A, B, C> extends TwoTuple<A, B> {
    private final C third;
 
    public ThreeTuple(A a, B b, C c) {
        super(a, b);
        this.third = c;
    }
 
    public C getThird() {
        return this.third;
    }
}