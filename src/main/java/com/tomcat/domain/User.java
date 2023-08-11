package com.tomcat.domain;

import lombok.Data;
import org.hibernate.annotations.GenericGenerator;

import javax.persistence.*;

@Data
@Entity
@Table(name = "user")
public class User {

  @Id
  @GeneratedValue
  private Long id;

  @Column(unique=true)
  private String username;

  private String password;
  @Column(unique=true)
  private String email;
  @Column(unique=true)
  private String phoneNum;

  private String deviceId;

  public User(Long id, String username, String password, String email, String phoneNum, String deviceId) {
    this.id = id;
    this.username = username;
    this.password = password;
    this.email = email;
    this.phoneNum = phoneNum;
    this.deviceId = deviceId;
  }

  public User() {

  }

  public String getDeviceId() {
    return deviceId;
  }

  public void setDeviceId(String deviceId) {
    this.deviceId = deviceId;
  }

  public String getPhoneNum() {
    return phoneNum;
  }

  public void setPhoneNum(String phoneNum) {
    this.phoneNum = phoneNum;
  }

  public Long getId() {
    return id;
  }

  public void setId(Long id) {
    this.id = id;
  }

  public String getUsername() {
    return username;
  }

  public void setUsername(String username) {
    this.username = username;
  }

  public String getPassword() {
    return password;
  }

  public void setPassword(String password) {
    this.password = password;
  }

  public String getEmail() {
    return email;
  }

  public void setEmail(String email) {
    this.email = email;
  }

}